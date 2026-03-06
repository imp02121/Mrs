//! sr-engine: School Run Strategy backtester, signal generator, and HTTP API.

use clap::{Parser, Subcommand};
use sr_engine::api::{AppState, api_routes};
use sr_engine::db::ValkeyCache;
use tracing::info;

/// School Run Strategy engine.
#[derive(Debug, Parser)]
#[command(name = "sr-engine", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Fetch historical OHLCV data from a provider.
    Fetch,
    /// Run a backtest with the given configuration.
    Backtest,
    /// Start the HTTP API server.
    Serve,
    /// Run pending database migrations.
    Migrate,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Fetch => {
            todo!("fetch: download historical candle data")
        }
        Command::Backtest => {
            todo!("backtest: run strategy over historical data")
        }
        Command::Serve => {
            let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
            let db_pool = sqlx::PgPool::connect(&database_url).await?;

            let cache = match std::env::var("VALKEY_URL") {
                Ok(url) => match ValkeyCache::new(&url).await {
                    Ok(c) => {
                        info!("connected to Valkey cache");
                        Some(c)
                    }
                    Err(e) => {
                        tracing::warn!("failed to connect to Valkey, running without cache: {e}");
                        None
                    }
                },
                Err(_) => None,
            };

            let state = AppState { db_pool, cache };
            let app = api_routes(state);

            let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
            let port = std::env::var("PORT").unwrap_or_else(|_| "3001".into());
            let addr = format!("{host}:{port}");

            info!("starting server on {addr}");
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(listener, app).await?;
            Ok(())
        }
        Command::Migrate => {
            let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
            let pool = sqlx::PgPool::connect(&database_url).await?;
            sqlx::migrate!("../migrations").run(&pool).await?;
            info!("migrations applied successfully");
            Ok(())
        }
    }
}
