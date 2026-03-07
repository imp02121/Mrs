# School Run Strategy

> Open-source trading strategy backtester based on Tom Hougaard's School Run Strategy

<!-- Badges -->
![CI](https://img.shields.io/badge/CI-passing-brightgreen?style=flat-square)
![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)
![Rust](https://img.shields.io/badge/rust-stable-orange?style=flat-square)

---

## What is this?

The School Run Strategy is a mechanical breakout trading strategy created by Tom Hougaard. It exploits a market microstructure observation: institutional market makers execute overnight client orders during the first 15 minutes after session open, creating noisy price action. The second 15-minute candle captures the transition to genuine directional conviction. A breakout beyond this candle's range signals the emerging trend for the session.

**School Run (`sr`)** is a signals-only backtesting engine and dashboard for this strategy. It does not connect to live brokers or execute real trades. Feed it historical 15-minute OHLCV candle data, configure the strategy parameters, and get back equity curves, drawdown analysis, trade-by-trade logs, and statistical summaries. A Telegram bot can push daily signal levels to subscribers.

## Features

- Backtesting engine with configurable parameters (stop loss modes, exit strategies, position sizing)
- Multi-instrument support: DAX, FTSE 100, Nasdaq 100, Dow Jones 30
- Three stop loss modes: signal bar extreme, fixed points, midpoint
- Multiple exit strategies: end-of-day, trailing stop, fixed take-profit, close-at-time
- Adding to winners with configurable intervals, max additions, and size multipliers
- Parameter sweep across multiple dimensions with parallel execution (Rayon)
- React dashboard for reviewing backtest results, comparing configurations, and viewing charts
- Telegram bot for daily signal bar levels and trade trigger notifications
- Precise financial arithmetic using `rust_decimal` (no floating-point rounding errors)
- All timestamps stored as UTC with DST-aware timezone conversions

## Quick Start

```bash
git clone https://github.com/<owner>/sr.git
cd sr
cargo build --workspace
```

## Docker Deployment

Each service runs as an independent Docker container. See [docs/deployment.md](docs/deployment.md) for the full deployment guide.

```bash
# Start infrastructure
docker run -d --name sr-postgres \
  -e POSTGRES_USER=sr -e POSTGRES_PASSWORD=sr_dev -e POSTGRES_DB=school_run \
  -p 5432:5432 postgres:16-alpine

docker run -d --name sr-valkey -p 6379:6379 valkey/valkey:8-alpine

# Build and run services
docker build -t sr-engine -f engine/Dockerfile .
docker run -d --name sr-engine \
  -e DATABASE_URL=postgres://sr:sr_dev@host.docker.internal:5432/school_run \
  -p 3001:3001 sr-engine

docker build -t sr-auth -f auth/Dockerfile .
docker run -d --name sr-auth \
  -e AUTH_DATABASE_URL=postgres://sr:sr_dev@host.docker.internal:5432/school_run \
  -e JWT_SECRET=change-me-to-a-random-string-at-least-32-bytes \
  -p 3002:3002 sr-auth

docker build -t sr-dashboard -f dashboard/Dockerfile dashboard/
docker run -d --name sr-dashboard -p 3000:80 sr-dashboard

# Run migrations and verify
docker exec sr-engine sr-engine migrate
curl http://localhost:3001/api/health
```

## Backtesting

The backtest engine runs the School Run Strategy over historical candle data and produces trade logs, equity curves, and statistical summaries.

### Running a Backtest (Rust API)

```rust
use sr_engine::backtest::{run_backtest, BacktestResult};
use sr_engine::models::Instrument;
use sr_engine::strategy::StrategyConfig;

let config = StrategyConfig {
    instrument: Instrument::Dax,
    date_from: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
    date_to: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
    ..StrategyConfig::default()
};

let result: BacktestResult = run_backtest(&candles, Instrument::Dax, &config);

println!("Trades: {}", result.trade_count());
println!("Final equity: {}", result.final_equity());
println!("{}", result.stats); // Sharpe, drawdown, win rate, etc.
```

### JSON Configuration

Strategy parameters can be deserialized from JSON:

```json
{
  "instrument": "Dax",
  "signal_bar_index": 2,
  "entry_offset_points": "2",
  "sl_mode": "FixedPoints",
  "sl_fixed_points": "40",
  "exit_mode": "EndOfDay",
  "exit_eod_time": "17:30:00",
  "date_from": "2024-01-01",
  "date_to": "2024-12-31",
  "initial_capital": "100000",
  "position_size": "1",
  "slippage_points": "0.5"
}
```

### Parameter Sweep

Sweep across multiple parameter values in parallel:

```rust
use sr_engine::backtest::{run_sweep, SweepConfig, best_by};

let sweep = SweepConfig {
    sl_fixed_points: vec![dec!(20), dec!(30), dec!(40), dec!(50)],
    entry_offset_points: vec![dec!(1), dec!(2), dec!(3)],
    ..Default::default()
};

let results = run_sweep(&candles, Instrument::Dax, &base_config, &sweep);

// Find the best result by Sharpe ratio
if let Some(best) = best_by(&results, |stats| stats.sharpe_ratio) {
    println!("Best Sharpe: {:.2}", best.result.stats.sharpe_ratio);
    println!("Config: SL={}, Offset={}", best.config.sl_fixed_points, best.config.entry_offset_points);
}
```

### Backtest Statistics

Each backtest produces a `BacktestStats` summary:

| Metric | Type | Description |
|---|---|---|
| `total_trades` | `u32` | Total completed trades |
| `win_rate` | `f64` | Fraction of winning trades |
| `total_pnl` | `Decimal` | Net profit/loss |
| `profit_factor` | `f64` | Gross wins / gross losses |
| `max_drawdown` | `Decimal` | Peak-to-trough equity drawdown |
| `max_drawdown_pct` | `f64` | Drawdown as percentage of peak |
| `sharpe_ratio` | `f64` | Annualised Sharpe (daily returns, rf=0) |
| `sortino_ratio` | `f64` | Annualised Sortino (downside only) |
| `calmar_ratio` | `f64` | Annualised return / max drawdown |
| `max_consecutive_wins` | `u32` | Longest winning streak |
| `max_consecutive_losses` | `u32` | Longest losing streak |
| `avg_trade_duration_minutes` | `f64` | Average time in trade |

See [docs/strategy.md](docs/strategy.md) for full parameter reference and strategy details.

## Architecture

School Run is a monorepo with five services:

- **Engine** (Rust / Axum) -- Core strategy logic, backtest engine, HTTP API. Accepts backtest requests, iterates over candle data, applies the School Run strategy rules, and returns trade logs and statistics.
- **Dashboard** (React / Vite / TypeScript) -- Web UI for configuring backtests, viewing equity curves, browsing trade logs, and inspecting signal bar charts. Communicates with the Engine via REST/JSON.
- **Telegram Bot** (Rust / Teloxide) -- Subscribes to signals from the Engine and pushes daily signal bar levels and trade triggers to Telegram subscribers.
- **Valkey** (Redis-compatible) -- Async write-behind buffer for real-time signal data and event queues. Decouples hot-path signal generation from durable storage.
- **PostgreSQL** -- Persistent storage for candle data, backtest runs, trade records, strategy configurations, and subscriber lists.

Each service runs as an independent Docker container. There is no docker-compose coupling; orchestration is left to the operator.

```
Browser (Dashboard)          Telegram Client
       |                            |
       | HTTP/JSON                  | Telegram Bot API
       v                            v
   Dashboard Service         Telegram Bot Service
       |                            |
       +--------+     +-------------+
                |     |
                v     v
           Engine Service (Rust/Axum)
                |           |
                v           v
             Valkey     PostgreSQL
```

## Data Pipeline

The engine fetches historical 15-minute OHLCV candle data from the [Twelve Data](https://twelvedata.com) API and stores it locally in Parquet files and optionally in PostgreSQL.

```
Twelve Data API  -->  DataFetcher  -->  ParquetStore (data/{instrument}/{YYYY-MM}.parquet)
                                   -->  PostgresStore (candles table, upsert)
```

**Setup:**

```bash
# Set your Twelve Data API key
export TWELVE_DATA_API_KEY=your_key_here

# Fetch 24 months of DAX data
sr-engine fetch --instrument DAX --months 24
```

The pipeline handles rate limiting (8 req/min on free tier), automatic pagination for long date ranges, exponential backoff on transient errors, and DST-aware timestamp normalization. See [data/README.md](data/README.md) for full details and [docs/data-pipeline.md](docs/data-pipeline.md) for architecture.

## Documentation

| Document | Description |
|---|---|
| [BUILD.md](BUILD.md) | Full build plan, architecture, and phase breakdown |
| [CLAUDE.md](CLAUDE.md) | AI agent coding conventions |
| [docs/strategy.md](docs/strategy.md) | Strategy guide (how the School Run strategy works) |
| [docs/api.md](docs/api.md) | API reference for the Engine HTTP endpoints |
| [docs/architecture.md](docs/architecture.md) | System architecture and design decisions |
| [docs/data-pipeline.md](docs/data-pipeline.md) | Data pipeline architecture and design |
| [docs/auth.md](docs/auth.md) | Auth service reference (OTP, JWT, rate limiting) |
| [docs/telegram.md](docs/telegram.md) | Telegram bot reference (commands, notifications) |
| [docs/deployment.md](docs/deployment.md) | Deployment guide and environment configuration |
| [data/README.md](data/README.md) | How to obtain and format historical candle data |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute (setup, style, PR workflow) |
| [CHANGELOG.md](CHANGELOG.md) | Version history and release notes |

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide covering development setup, code style, commit conventions, and pull request workflow.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Disclaimer

This software is provided for **educational and research purposes only**. It is not financial advice. It does not execute real trades and should not be used as the sole basis for any trading decisions. Past backtest performance does not guarantee future results. Trading financial instruments involves significant risk of loss.

This project is **not affiliated with, endorsed by, or associated with Tom Hougaard** in any way. It is an independent open-source implementation of a publicly described mechanical trading strategy. The strategy description is based on publicly available materials. All rights to the original strategy concept belong to their respective owners.

Use this software at your own risk. The authors accept no liability for financial losses incurred through the use of this tool.
