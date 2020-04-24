//! I/O related code.

use async_std::{io, task};
use async_std::io::ReadExt;
use async_std::io::prelude::WriteExt;
use async_std::net::{SocketAddr, IpAddr, Ipv4Addr, TcpListener, TcpStream};
use async_std::stream::StreamExt;

use crate::proxy;

/// Listen for incoming connections on a given TCP port.
/// Spawns an async task for each connection.
pub async fn run_server(port: u16) -> io::Result<()> {
    let listen_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);
    let listener = TcpListener::bind(listen_addr).await?;
    let mut incoming = listener.incoming();

    println!("Listening for connections on port {}", port);

    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        let _ = task::spawn(handle_connection(stream));
    }

    Ok(())
}

pub async fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    println!("New incoming connection: {}", stream.peer_addr().unwrap());
    let mut proxy_sess = proxy::Session::waiting_request();
    let mut buf = vec![0u8; 4096];

    loop {
        let bytes_read = stream.read(&mut buf).await?;
        if bytes_read == 0 {
            break;
        }

        proxy_sess = proxy_sess.on_data(&buf[..bytes_read]);
        println!("{:?}", proxy_sess);
        proxy_sess = match proxy_sess {
            proxy::Session::OnConnectRequest(state) => {
                let mut out_stream = TcpStream::connect(&state.path).await?;
                // TODO(povilas): get this from ConnectTunnel state.
                stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;
                proxy::Session::ConnectTunnel(state.on_connected(out_stream))
            }
            proxy::Session::ConnectTunnel(mut state) => {
                state.conn.write_all(&buf[..bytes_read]).await?;
                let bytes_read = state.conn.read(&mut buf).await?;
                stream.write_all(&buf[..bytes_read]).await?;
                proxy::Session::ConnectTunnel(state)
            }
            proxy::Session::Failed(_state) => {
                println!("State machine failed");
                return Ok(());
            }
            proxy_sess => proxy_sess,
        }
    }

    Ok(())
}
