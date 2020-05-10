//! I/O related code.

use std::future::{Future};
use std::pin::Pin;
use std::task::{Poll, Context};
use std::collections::VecDeque;

use async_std::{io, task};
use async_std::io::ReadExt;
use async_std::io::prelude::WriteExt;
use async_std::net::{SocketAddr, IpAddr, Ipv4Addr, TcpListener, TcpStream};
use async_std::stream::StreamExt;
use httparse::Status;
use futures::{TryFutureExt, pin_mut};
use unwrap::unwrap;
use err_derive::Error;
use log::info;


#[derive(Debug, Error)]
enum Error {
    #[error(display = "I/O error")]
    Io(#[source] io::Error),
    #[error(display = "Unexpected HTTP request (expected {}, got {})", _0, _1)]
    UnexpectedHttpRequest(String, String),
    #[error(display="HTTP issue: {}", _0)]
    Http(String),
}


/// Listen for incoming connections on a given TCP port.
/// Spawns an async task for each connection.
pub async fn run_server(port: u16) -> io::Result<()> {
    let listen_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);
    let listener = TcpListener::bind(listen_addr).await?;
    let mut incoming = listener.incoming();

    info!("Listening for connections on port {}", port);

    while let Some(Ok(stream)) = incoming.next().await {
        let _ = task::spawn(handle_connection(stream).map_err(|e| {
            info!("Error handling connection: {}", e);
            ()
        }));
    }

    Ok(())
}

struct ProxyData {
    stream1: TcpStream,
    stream2: TcpStream,
    stream1_in_buff: VecDeque<Vec<u8>>,
    stream2_in_buff: VecDeque<Vec<u8>>,
}

impl ProxyData {
    fn new(stream1: TcpStream, stream2: TcpStream) -> Self {
        Self {
            stream1,
            stream2,
            stream1_in_buff: Default::default(),
            stream2_in_buff: Default::default()
        }
    }
}

/// Reduces duplicate code when forwarding data
/// stream1 ---> stream2 and then stream2 ---> stream1.
///
/// Was meaning to write a function, but Rustc was not happy about split borrowing
/// from `ProxyData`.
macro_rules! forward_data {
    ( $from:expr, $to:expr, $from_buff:expr, $cx:expr ) => {
        let mut buff = [0u8; 65535];

        loop {
            let fut = $from.read(&mut buff);
            pin_mut!(fut);

            match fut.poll($cx) {
                Poll::Pending => break,
                Poll::Ready(res) => {
                    let bytes_read = res?;
                    if bytes_read == 0 {
                        return Poll::Ready(Ok(()));
                    }

                    $from_buff.push_back(buff[..bytes_read].to_vec())
                },
            }
        }

        while let Some(buff) = $from_buff.pop_front() {
            let fut = $to.write(&buff);
            pin_mut!(fut);

            match fut.poll($cx) {
                Poll::Pending => break,
                Poll::Ready(res) => {
                    let bytes_written = res?;
                    if bytes_written < buff.len() {
                        $from_buff.push_front(buff[bytes_written..].to_vec());
                    }
                }
            }
        }

    }
}

impl Future for ProxyData {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        forward_data!(&mut self.stream1, &mut self.stream2, &mut self.stream1_in_buff, cx);
        forward_data!(&mut self.stream2, &mut self.stream1, &mut self.stream2_in_buff, cx);
        Poll::Pending
    }
}

pub async fn handle_connection(mut client_conn: TcpStream) -> Result<(), Error> {
    info!("New incoming connection: {}", client_conn.peer_addr().unwrap());

    let mut buf = vec![0u8; 65535];

    let bytes_read = client_conn.read(&mut buf).await?;
    if bytes_read == 0 {
        return Err(Error::Io(io::ErrorKind::ConnectionReset.into()));
    }

    let connect_to = parse_conn_request(&buf[..bytes_read])?;
    info!("Connecting to: {}", connect_to);

    let target_conn = TcpStream::connect(connect_to).await?;
    info!("...connected");

    client_conn.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;

    ProxyData::new(client_conn, target_conn).await;

    Ok(())
}

fn parse_conn_request(data: &[u8]) -> Result<String, Error> {
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut req = httparse::Request::new(&mut headers);

    match unwrap!(req.parse(data)) {
        Status::Complete(_bytes_parsed) => {
            if let (Some(method), Some(path)) = (req.method, req.path) {
                if method == "CONNECT" {
                    Ok(path.to_string())
                } else {
                    Err(Error::UnexpectedHttpRequest(
                            "CONNECT".to_string(), method.to_string()))
                }
            } else {
                Err(Error::Http("Couldn't parse method and path".to_string()))
            }
        }
        Status::Partial => {
            Err(Error::Http(
                "HTTP request was partially parsed - not yet supported!".to_string()))
        }
    }
}
