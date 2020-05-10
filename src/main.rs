mod proxy;
mod io;

use log::Level;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    simple_logger::init_with_level(Level::Info).unwrap();

    io::run_server(8081).await?;
    Ok(())
}
