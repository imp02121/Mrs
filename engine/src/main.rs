//! sr-engine: School Run Strategy backtester, signal generator, and HTTP API.

use clap::{Parser, Subcommand};

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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Fetch => {
            todo!("fetch: download historical candle data")
        }
        Command::Backtest => {
            todo!("backtest: run strategy over historical data")
        }
        Command::Serve => {
            todo!("serve: start axum HTTP API server")
        }
        Command::Migrate => {
            todo!("migrate: run pending database migrations")
        }
    }
}
