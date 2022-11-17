use galoy_bitcoin::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::run().await
}
