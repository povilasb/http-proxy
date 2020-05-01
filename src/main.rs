mod proxy;
mod io;

use unwrap::unwrap;
use httparse;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    io::run_server(8080).await?;
    Ok(())
}
