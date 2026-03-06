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

> Coming soon. The project is under active development.
>
> Once Phase 0 is complete, the quick start will cover:
>
> ```bash
> git clone https://github.com/<owner>/sr.git
> cd sr
> cargo build --workspace
> cd dashboard && npm install && npm run dev
> ```

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
| [docs/deployment.md](docs/deployment.md) | Deployment guide and environment configuration |
| [data/README.md](data/README.md) | How to obtain and format historical candle data |

## Contributing

Contributions are welcome. A full contributing guide (`CONTRIBUTING.md`) is planned for Phase 8. In the meantime:

1. Fork the repository
2. Create a feature branch (`feat/your-feature`)
3. Follow the conventions in [CLAUDE.md](CLAUDE.md)
4. Ensure `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` pass
5. Open a pull request with a clear description

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Disclaimer

This software is provided for **educational and research purposes only**. It is not financial advice. It does not execute real trades and should not be used as the sole basis for any trading decisions. Past backtest performance does not guarantee future results. Trading financial instruments involves significant risk of loss.

This project is **not affiliated with, endorsed by, or associated with Tom Hougaard** in any way. It is an independent open-source implementation of a publicly described mechanical trading strategy. The strategy description is based on publicly available materials. All rights to the original strategy concept belong to their respective owners.

Use this software at your own risk. The authors accept no liability for financial losses incurred through the use of this tool.
