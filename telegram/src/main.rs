//! sr-telegram: Telegram notification bot for the School Run Strategy.
//!
//! Entry point that loads configuration, connects to PostgreSQL,
//! creates the Teloxide bot, spawns the signal polling loop, and
//! runs the command dispatcher.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use teloxide::prelude::*;

use sr_telegram::commands::{Command, HandlerDeps};
use sr_telegram::config::Config;
use sr_telegram::signals;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file (ignore if missing)
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("sr-telegram starting...");

    let config = Config::from_env()?;

    // Connect to PostgreSQL
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;
    tracing::info!("connected to database");

    // Create the Telegram bot
    let bot = Bot::new(&config.bot_token);

    // Create shared dependencies for command handlers
    let deps = Arc::new(HandlerDeps {
        pool: pool.clone(),
        http_client: reqwest::Client::new(),
        engine_api_url: config.engine_api_url.clone(),
    });

    // Spawn the signal polling loop in the background
    let signal_config = config.clone();
    let signal_pool = pool.clone();
    let signal_bot = bot.clone();
    tokio::spawn(async move {
        signals::run_signal_loop(signal_config, signal_pool, signal_bot).await;
    });

    tracing::info!("signal polling loop started");

    // Build the command handler using dptree + Dispatcher
    let handler = Update::filter_message()
        .filter_command::<Command>()
        .endpoint(
            |bot: Bot, msg: Message, cmd: Command, deps: Arc<HandlerDeps>| async move {
                sr_telegram::commands::handle_command(bot, msg, cmd, deps).await
            },
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![deps])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
