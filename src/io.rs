//! I/O related code.

use async_std::{io, task};
use async_std::io::ReadExt;
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

        proxy_sess = proxy_sess.on_data(proxy::Data(buf[..bytes_read].to_vec()));
        println!("{:?}", proxy_sess);
        match proxy_sess {
            proxy::Session::OnConnectRequest(ref state) => {
                let out_stream = TcpStream::connect(&state.path).await?;
                println!("Connected: {:?}", out_stream);
            }
            _ => (),
        }
    }

    Ok(())
}
