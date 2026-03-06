//! sr-telegram: Telegram notification bot for the School Run Strategy.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("sr-telegram starting...");
    Ok(())
}
