mod proxy;
mod io;

use unwrap::unwrap;
use httparse;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    /*
    let buf = b"CONNECT httpbin.org:443 HTTP/1.1\r\nHost: httpbin.org:443\r\n\r\nTLS handshake".to_vec();
    let s = s.on_data(proxy::Data(buf));
    let s = s.on_connected(proxy::Connected);
    println!("{:?}", s);
    */
    io::run_server(8080).await?;
    Ok(())
}
