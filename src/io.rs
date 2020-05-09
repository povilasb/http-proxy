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
use futures::pin_mut;
use unwrap::unwrap;


/// Listen for incoming connections on a given TCP port.
/// Spawns an async task for each connection.
pub async fn run_server(port: u16) -> io::Result<()> {
    let listen_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);
    let listener = TcpListener::bind(listen_addr).await?;
    let mut incoming = listener.incoming();

    println!("Listening for connections on port {}", port);

    while let Some(Ok(stream)) = incoming.next().await {
        let _ = task::spawn(handle_connection(stream));
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

impl Future for ProxyData {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut buff = [0u8; 65535];

        // stream1 --> stream2
        loop {
            let fut = self.stream1.read(&mut buff);
            pin_mut!(fut);

            match fut.poll(cx) {
                Poll::Pending => break,
                Poll::Ready(res) => {
                    let bytes_read = res?;
                    if bytes_read == 0 {
                        return Poll::Ready(Ok(()));
                    }

                    self.stream1_in_buff.push_back(buff[..bytes_read].to_vec())
                },
            }
        }

        while let Some(buff) = self.stream1_in_buff.pop_front() {
            let fut = self.stream2.write(&buff);
            pin_mut!(fut);

            match fut.poll(cx) {
                Poll::Pending => break,
                Poll::Ready(res) => {
                    let bytes_written = res?;
                    if bytes_written < buff.len() {
                        self.stream1_in_buff.push_front(buff[bytes_written..].to_vec());
                    }
                }
            }
        }

        // stream2 --> stream1
        loop {
            let fut = self.stream2.read(&mut buff);
            pin_mut!(fut);

            match fut.poll(cx) {
                Poll::Pending => break,
                Poll::Ready(res) => {
                    let bytes_read = res?;
                    if bytes_read == 0 {
                        return Poll::Ready(Ok(()));
                    }

                    self.stream2_in_buff.push_back(buff[..bytes_read].to_vec())
                },
            }
        }

        while let Some(buff) = self.stream2_in_buff.pop_front() {
            let fut = self.stream1.write(&buff);
            pin_mut!(fut);

            match fut.poll(cx) {
                Poll::Pending => break,
                Poll::Ready(res) => {
                    let bytes_written = res?;
                    if bytes_written < buff.len() {
                        self.stream2_in_buff.push_front(buff[bytes_written..].to_vec());
                    }
                }
            }
        }

        Poll::Pending
    }
}

pub async fn handle_connection(mut client_conn: TcpStream) -> io::Result<()> {
    println!("New incoming connection: {}", client_conn.peer_addr().unwrap());

    let mut buf = vec![0u8; 65535];

    let bytes_read = client_conn.read(&mut buf).await?;
    if bytes_read == 0 {
        // TODO(povilas): return error instead with conn reset or smth
        return Ok(());
    }

    // TODO(povilas): return error if not connect
    let connect_to = parse_conn_request(&buf[..bytes_read]);
    println!("Connecting to: {}", connect_to);

    let target_conn = TcpStream::connect(connect_to).await?;
    println!("...connected");

    client_conn.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;

    ProxyData::new(client_conn, target_conn).await;

    Ok(())
}

// TODO(povilas): return result
fn parse_conn_request(data: &[u8]) -> String {
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut req = httparse::Request::new(&mut headers);

    // TODO(povilas): return error
    match unwrap!(req.parse(data)) {
        Status::Complete(_bytes_parsed) => {
            if let (Some(method), Some(path)) = (req.method, req.path) {
                if method == "CONNECT" {
                    return path.to_string()
                } else {
                    panic!("unsupported method: {}", method);
                }
            } else {
                panic!("Couldn't parse method and path");
            }
        }
        Status::Partial => {
            panic!("HTTP request was partially parsed - not yet supported!");
        }
    }
}
