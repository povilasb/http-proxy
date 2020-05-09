mod proxy;
mod io;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    io::run_server(8081).await?;
    Ok(())
}
