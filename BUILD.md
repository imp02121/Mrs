# School Run Strategy - Build Plan

> A mechanical breakout trading strategy engine based on Tom Hougaard's School Run Strategy.
> Open-source. Rust engine + React dashboard + Telegram bot.

---

## 1. Project Overview

The **School Run Strategy** is a mechanical breakout trading strategy created by Tom Hougaard (TraderTom). The strategy is documented in his December 2022 publication *"School Run Strategy December 2022"* and is rooted in a specific market microstructure observation: market makers execute client orders during the first 30 minutes of the trading session, and the "real" directional trend emerges after this period.

### Core Thesis

Institutional market makers receive overnight and pre-market orders from clients that must be executed at the open. This creates noisy, often misleading price action in the first 15 minutes. The **second** 15-minute candle after market open captures the transition from market-maker-driven flow to genuine directional conviction. A breakout beyond this candle's range signals the emerging trend.

### Strategy Mechanics

1. **Signal Bar Identification**: On a 15-minute chart, identify the 2nd candle after the market open. For example, the DAX opens at 09:00 CET, so the signal bar spans 09:15 to 09:30 CET.

2. **Entry Orders**:
   - Place a **buy stop** order 2 points above the signal bar's high.
   - Place a **sell stop** order 2 points below the signal bar's low.
   - Both orders remain active for the session. Both sides can trigger in the same session.

3. **Stop Loss** (three documented modes):
   - **Signal Bar Extreme**: Stop placed at the opposite extreme of the signal bar (can be large).
   - **Fixed Points**: A fixed-distance stop (Hougaard used 40 points when DAX traded near 12,000; this should scale with index level).
   - **Midpoint**: Stop placed at the halfway point of the signal bar, with discretionary offset.

4. **Exit Strategy**: Hougaard explicitly states he does not use a fixed exit rule -- he reads the chart. For a mechanical backtesting system, we parameterize exits as: end-of-day close, trailing stop, fixed take-profit, or close-at-time.

5. **Adding to Winners**: When a trade moves into profit and pulls back before resuming, Hougaard describes doubling up and tightening the stop. For backtesting, this is parameterized as: add every X points of favorable movement, with a maximum number of additions and a configurable size multiplier per add.

6. **Applicable Instruments**: DAX (primary, most documented), FTSE 100, Nasdaq 100, Dow Jones 30. Each has a different session open time and different volatility characteristics.

### What This Project Is

**School Run (`sr`)** is an open-source, signals-only implementation of the School Run Strategy with a heavy backtesting focus. It does **not** connect to live brokers or execute real trades. Its purposes are:

- **Backtesting**: Run the strategy across years of historical 15-minute OHLCV data with configurable parameters. Sweep parameter spaces. Generate equity curves, drawdown analysis, and trade-by-trade logs.
- **Signal Generation**: Emit real-time signals (buy stop / sell stop levels, triggers, stop levels) for the current session based on live or delayed data.
- **Visualization**: A web dashboard for reviewing backtest results, comparing parameter configurations, and viewing signal bar charts.
- **Notifications**: A Telegram bot that pushes signal bar levels and trade triggers to subscribers.

### What This Project Is Not

- Not a trading bot. It will never place orders with a broker.
- Not financial advice software. It is a research and education tool.
- Not affiliated with Tom Hougaard. It is an independent open-source implementation of a publicly described mechanical strategy.

---

## 2. Architecture Overview

```
+---------------------------------------------------------------------+
|                         EXTERNAL USERS                              |
|                                                                     |
|    +---------------+                      +--------------------+    |
|    |   Browser      |                      |  Telegram Client   |    |
|    |  (Dashboard)   |                      |  (Mobile/Desktop)  |    |
|    +-------+--------+                      +---------+----------+    |
+------------|-----------------------------------------|---------------+
             | HTTP (REST/JSON)                        | Telegram Bot API
             |                                         |
+------------v--------------------+    +---------------v-----------------+
|                                 |    |                                 |
|     DASHBOARD SERVICE           |    |      TELEGRAM BOT SERVICE      |
|     (React + Vite + TS)         |    |      (Rust / Teloxide)         |
|                                 |    |                                 |
|  - Backtest config UI           |    |  - Subscribes to signals       |
|  - Equity curve charts          |    |    from Engine via HTTP        |
|  - Trade log viewer             |    |    polling / Valkey pubsub     |
|  - Signal bar chart             |    |  - Push signal bar levels      |
|                                 |    |  - Push trade triggers         |
|  Dockerfile (standalone)        |    |  - Command interface           |
+------------+--------------------+    |                                 |
             |                         |  Dockerfile (standalone)        |
             |  HTTP calls             +---------------+-----------------+
             |                                         |
             |                                         |  HTTP calls + writes
+------------v-----------------------------------------v-----------------+
|                                                                       |
|                      ENGINE SERVICE (Rust / Axum)                     |
|                                                                       |
|  +---------------+  +---------------+  +--------------------------+  |
|  |  Strategy      |  |  Backtest     |  |  HTTP API               |  |
|  |  Module        |  |  Engine       |  |                          |  |
|  |                |  |               |  |  POST /backtest/run      |  |
|  |  - Signal bar  |  |  - Candle     |  |  GET  /backtest/{id}     |  |
|  |    detection   |  |    iteration  |  |  GET  /signals/today     |  |
|  |  - Order       |  |  - Parameter  |  |  POST /config            |  |
|  |    placement   |  |    sweeps     |  |  GET  /health            |  |
|  |  - SL/TP/Exit  |  |  - Stats      |  |                          |  |
|  |  - Adds        |  |    calc       |  |                          |  |
|  +---------------+  +---------------+  +--------------------------+  |
|                                                                       |
|  Dockerfile (standalone)                                              |
+------------------+----------------------------+-----------------------+
                   |                            |
                   |  Non-blocking writes       |  Non-blocking writes
                   |                            |
+------------------v----------------------------v-----------------------+
|                                                                       |
|                    VALKEY (Redis-compatible)                           |
|                                                                       |
|  - signal:{date}:{instrument}  -> signal bar levels, triggers        |
|  - trades:pending              -> trade events queue                 |
|  - backtest:results:{id}       -> interim backtest progress          |
|  - config:active               -> current running config             |
|                                                                       |
+-------------------------------+---------------------------------------+
                                |
                                |  Async flush (background worker
                                |  in Engine or standalone cron)
                                |
+-------------------------------v---------------------------------------+
|                                                                       |
|                       POSTGRESQL                                      |
|                                                                       |
|  backtest_runs      - run ID, params, status, summary stats          |
|  trades             - per-trade P&L, entry/exit, instrument          |
|  strategy_configs   - saved parameter sets                           |
|  candles            - historical OHLCV (partitioned by month)        |
|  live_signals       - daily signal bar records                       |
|  subscribers        - Telegram bot subscribers                       |
|                                                                       |
+-----------------------------------------------------------------------+
```

### Data Flow Summary

1. **Dashboard to Engine**: The React dashboard makes HTTP requests to the Engine API to submit backtest runs, retrieve results, fetch today's signal levels, and browse trade history.

2. **Telegram Bot to Engine**: The Telegram bot polls the Engine API (or receives Valkey pub/sub) for new signals and trade triggers, then pushes formatted messages to subscribed Telegram users/groups.

3. **Engine to Valkey**: Both the Engine and the Telegram bot write events to Valkey using non-blocking async calls. This ensures the hot path (signal generation, backtest iteration) is never blocked by database I/O. Valkey acts as a write-ahead buffer.

4. **Valkey to PostgreSQL**: A background worker (running inside the Engine process as a Tokio task) periodically flushes data from Valkey streams/lists into PostgreSQL for durable, queryable storage. Writes are eventually consistent but reads from the API can optionally hit Valkey for real-time data or PostgreSQL for historical queries.

5. **Independent containers**: Each service (Engine, Dashboard, Telegram, Valkey, PostgreSQL) has its own independent Dockerfile and is run as a standalone container. No docker-compose coupling. Orchestration is left to the operator.

---

## 3. Tech Stack

### Rust Engine

| Technology | Role | Rationale |
|---|---|---|
| **Rust (stable)** | Core language | Backtesting iterates over millions of candles with complex per-bar logic. Rust provides zero-cost abstractions, no GC pauses, and predictable latency. A parameter sweep across years of data must complete in seconds. Safety guarantees reduce bugs in financial logic. Strong open-source appeal. |
| **Axum** | HTTP API framework | Built on Tokio and Tower. Modern, ergonomic, composable. Excellent async support, strong typing with extractors, first-class Tokio ecosystem integration. Simpler mental model than Actix-web. |
| **Tokio** | Async runtime | De facto standard. Required by Axum, SQLx, redis-rs, and Teloxide. |
| **SQLx** | PostgreSQL driver | Async, pure-Rust. **Compile-time checked SQL queries** via `sqlx::query!` macro -- catches schema drift before code runs. |
| **redis-rs** | Valkey client | Async Redis client compatible with Valkey. Connection pooling, pipelining, Streams support. |
| **serde / serde_json** | Serialization | Industry-standard. JSON API bodies, config files, Valkey value encoding. |
| **chrono / chrono-tz** | Date/time handling | Timezone-aware date/time. Critical for CET/EST/UTC conversions and DST transitions. |
| **clap** | CLI argument parsing | For running engine in CLI mode (`sr-engine backtest --config params.toml`). |
| **tracing** | Structured logging | Async-aware, span support, integrates with Tokio. |
| **rust_decimal** | Decimal arithmetic | Precise financial arithmetic without f64 rounding issues. |
| **rayon** | Data parallelism | For parameter sweeps -- each combination runs independently on its own thread. |

### React Dashboard

| Technology | Role | Rationale |
|---|---|---|
| **React 19** | UI framework | Component model ideal for backtest config forms, result tables, chart panels. |
| **Vite** | Build tool | Sub-second HMR, native ESM dev server, fast production builds. |
| **TypeScript** | Language | Catches API contract mismatches at compile time, IDE autocompletion. |
| **TanStack Query** | Server state | Request caching, deduplication, background refetch, loading/error states. |
| **Lightweight Charts** | Candlestick charts | TradingView open-source lib. Purpose-built for financial charts. ~40KB gzipped. |
| **Recharts** | Analytics charts | Line charts (equity curves), bar charts (PnL distribution), area charts (drawdown). |
| **Tailwind CSS** | Styling | Utility-first, rapid UI development, consistent design system. |
| **Zustand** | Client state | Minimal boilerplate, no providers, fine-grained subscriptions. |
| **TanStack Table** | Data tables | Headless sorting, filtering, pagination for trade log tables. |

### Telegram Bot

| Technology | Role | Rationale |
|---|---|---|
| **Teloxide** | Bot framework | Idiomatic Rust, built on Tokio. Keeps entire backend in one language. Share types via Cargo workspace. |

### Infrastructure

| Technology | Role | Rationale |
|---|---|---|
| **PostgreSQL 16** | Persistent storage | ACID transactions, rich query capabilities, JSON column support, table partitioning. |
| **Valkey 8** | Async write buffer | Open-source Redis fork (Linux Foundation). Non-blocking write buffer + pub/sub for real-time signals. |
| **Docker** | Containerization | Each service gets its own Dockerfile. Independent deploy/update per service. |

---

## 4. Project Structure

```
sr/
|
+-- BUILD.md                          # This file
+-- README.md                         # Project introduction, quick start
+-- Cargo.toml                        # Workspace root
+-- .gitignore
+-- .env.example
+-- LICENSE                           # MIT
|
+-- engine/                           # Rust: core strategy + backtester + HTTP API
|   +-- Dockerfile
|   +-- Cargo.toml
|   +-- config/
|   |   +-- default.toml              # Default engine config (port, DB URLs, log level)
|   |   +-- instruments.toml          # Per-instrument session times, tick sizes
|   +-- src/
|       +-- main.rs                   # Entry point: CLI (clap) + Axum server
|       +-- lib.rs                    # Re-exports
|       |
|       +-- strategy/                 # Signal bar detection and order management
|       |   +-- mod.rs
|       |   +-- signal_bar.rs         # Identify Nth 15-min candle after session open
|       |   +-- orders.rs             # Buy stop / sell stop placement
|       |   +-- stop_loss.rs          # SL modes: extreme, fixed, midpoint
|       |   +-- exit.rs               # Exit modes: EOD, trailing, TP, close-at-time
|       |   +-- add_to_winners.rs     # Adding logic: intervals, max adds, trailing
|       |
|       +-- backtest/                 # Backtest engine and parameter handling
|       |   +-- mod.rs
|       |   +-- engine.rs             # Core loop: iterate candles, apply strategy
|       |   +-- params.rs             # BacktestParams struct
|       |   +-- sweep.rs              # Parameter sweep: cartesian product, parallel
|       |   +-- stats.rs              # Post-run statistics
|       |   +-- report.rs             # Serialize results to JSON
|       |
|       +-- api/                      # Axum HTTP API
|       |   +-- mod.rs
|       |   +-- router.rs             # Route definitions
|       |   +-- handlers/
|       |   |   +-- backtest.rs       # POST /backtest/run, GET /backtest/{id}
|       |   |   +-- signals.rs        # GET /signals/today
|       |   |   +-- config.rs         # CRUD for strategy configs
|       |   |   +-- data.rs           # GET /data/candles, POST /data/fetch
|       |   |   +-- health.rs         # GET /health
|       |   +-- errors.rs             # Unified error types
|       |   +-- middleware.rs         # CORS, logging, request ID
|       |
|       +-- db/                       # Database integration
|       |   +-- mod.rs
|       |   +-- postgres.rs           # SQLx connection pool, query functions
|       |   +-- valkey.rs             # Redis-rs connection pool, stream helpers
|       |   +-- flush.rs              # Background Tokio task: Valkey -> Postgres
|       |
|       +-- data/                     # Data ingestion and parsing
|       |   +-- mod.rs
|       |   +-- provider.rs           # DataProvider trait
|       |   +-- twelve_data.rs        # Twelve Data API implementation
|       |   +-- fetcher.rs            # Backfill + incremental fetch orchestration
|       |   +-- parquet_store.rs      # Read/write Parquet files
|       |   +-- postgres_store.rs     # Bulk insert into Postgres
|       |
|       +-- models/                   # Shared domain types
|           +-- mod.rs
|           +-- candle.rs             # Candle { timestamp, open, high, low, close, volume }
|           +-- instrument.rs         # Instrument enum + session metadata
|           +-- signal.rs             # Signal bar type
|           +-- trade.rs              # Trade result type
|           +-- position.rs           # Position tracking
|           +-- config.rs             # Configuration structs
|
+-- dashboard/                        # React + Vite + TypeScript
|   +-- Dockerfile
|   +-- package.json
|   +-- tsconfig.json
|   +-- vite.config.ts
|   +-- tailwind.config.js
|   +-- nginx.conf                    # Production nginx config
|   +-- index.html
|   +-- src/
|       +-- main.tsx
|       +-- App.tsx
|       +-- pages/
|       |   +-- BacktestPage.tsx      # Config form + results display
|       |   +-- ChartPage.tsx         # Candlestick chart with signal overlays
|       |   +-- ComparePage.tsx       # Side-by-side backtest comparison
|       |   +-- SignalsPage.tsx       # Today's signal bar levels
|       |   +-- HistoryPage.tsx       # Past backtest runs
|       +-- components/
|       |   +-- layout/
|       |   |   +-- AppShell.tsx
|       |   |   +-- Sidebar.tsx
|       |   +-- backtest/
|       |   |   +-- ParameterPanel.tsx
|       |   |   +-- StatsCard.tsx
|       |   |   +-- EquityCurve.tsx
|       |   |   +-- MonthlyHeatmap.tsx
|       |   |   +-- TradeDistribution.tsx
|       |   |   +-- TradeTable.tsx
|       |   +-- chart/
|       |   |   +-- CandlestickChart.tsx
|       |   |   +-- SignalOverlay.tsx
|       |   |   +-- TradeMarkers.tsx
|       |   +-- signals/
|       |   |   +-- SignalCard.tsx
|       |   |   +-- SignalStatusBadge.tsx
|       |   +-- shared/
|       |       +-- InstrumentSelector.tsx
|       |       +-- DateRangePicker.tsx
|       +-- hooks/
|       |   +-- useBacktest.ts
|       |   +-- useConfigs.ts
|       |   +-- useCandles.ts
|       |   +-- useSignals.ts
|       +-- api/
|       |   +-- client.ts             # Axios instance with base URL
|       |   +-- endpoints.ts          # Typed API functions
|       +-- stores/
|       |   +-- backtest-store.ts     # Current config + active run ID
|       |   +-- ui-store.ts           # Sidebar, theme
|       +-- types/
|           +-- strategy.ts           # Mirrors Rust config types
|           +-- backtest.ts           # BacktestResult, BacktestStats
|           +-- signal.ts
|
+-- telegram/                         # Rust Telegram bot
|   +-- Dockerfile
|   +-- Cargo.toml
|   +-- src/
|       +-- main.rs                   # Bot startup, command registration
|       +-- commands.rs               # /start, /signals, /subscribe, /status
|       +-- notifications.rs          # Push signal bar levels and trade triggers
|       +-- signals.rs                # Poll engine API / Valkey pub/sub
|       +-- store.rs                  # Subscriber list management (Valkey-backed)
|       +-- config.rs                 # Env var configuration
|
+-- migrations/                       # PostgreSQL migrations (shared)
|   +-- 001_create_instruments.sql
|   +-- 002_create_candles.sql
|   +-- 003_create_strategy_configs.sql
|   +-- 004_create_backtest_runs.sql
|   +-- 005_create_trades.sql
|   +-- 006_create_live_signals.sql
|   +-- 007_create_subscribers.sql
|
+-- data/                             # Historical OHLCV data (GITIGNORED)
|   +-- .gitkeep
|   +-- README.md                     # Instructions on obtaining data
|
+-- docs/                             # Reference material
    +-- School Run Strategy December 2022.pdf
    +-- api.md
    +-- strategy-notes.md
```

---

## 5. Configurable Parameters

All parameters are defined in the `BacktestParams` struct and accepted as JSON in the `POST /backtest/run` request body.

### Signal Detection

| Parameter | Type | Default | Description |
|---|---|---|---|
| `instrument` | `Enum` | `DAX` | Target index: `DAX`, `FTSE`, `NQ`, `DOW`. Determines session open time. |
| `signal_bar_index` | `u8` | `2` | Which 15-min candle after open to use. Hougaard uses `2`. |
| `candle_interval_minutes` | `u16` | `15` | Candle timeframe. Allows experimentation with 5/10/30 min. |
| `entry_offset_points` | `f64` | `2.0` | Points above high / below low for entry stops. |
| `allow_both_sides` | `bool` | `true` | Whether both buy and sell stops can trigger same session. |

### Stop Loss

| Parameter | Type | Default | Description |
|---|---|---|---|
| `sl_mode` | `Enum` | `FixedPoints` | `SignalBarExtreme`, `FixedPoints`, or `Midpoint`. |
| `sl_fixed_points` | `f64` | `40.0` | Fixed SL distance. Only when `sl_mode = FixedPoints`. |
| `sl_midpoint_offset` | `f64` | `5.0` | Buffer beyond midpoint. Only when `sl_mode = Midpoint`. |
| `sl_scale_with_index` | `bool` | `false` | Scale SL proportionally to current index level. |
| `sl_scale_baseline` | `f64` | `12000.0` | Index level at which `sl_fixed_points` was calibrated. |

### Exit Strategy

| Parameter | Type | Default | Description |
|---|---|---|---|
| `exit_mode` | `Enum` | `EndOfDay` | `EndOfDay`, `TrailingStop`, `FixedTakeProfit`, `CloseAtTime`, `None`. |
| `exit_eod_time` | `String` | `"17:30"` | Time to flatten positions for EOD mode. |
| `trailing_stop_distance` | `f64` | `30.0` | Trail distance in points. |
| `trailing_stop_activation` | `f64` | `0.0` | Min unrealized profit before trailing activates. |
| `fixed_tp_points` | `f64` | `100.0` | Take profit distance in points. |
| `close_at_time` | `String` | `"15:00"` | Time to close all positions. |

### Adding to Winners

| Parameter | Type | Default | Description |
|---|---|---|---|
| `add_to_winners_enabled` | `bool` | `false` | Whether to add to winning positions. |
| `add_every_points` | `f64` | `50.0` | Add every X points of favorable movement. |
| `max_additions` | `u8` | `3` | Maximum additional entries per trade. |
| `add_size_multiplier` | `f64` | `1.0` | Size of each add relative to initial. `1.0` = same, `2.0` = double. |
| `move_sl_on_add` | `bool` | `true` | Tighten SL when adding. |
| `add_sl_offset` | `f64` | `0.0` | Offset from previous add's entry for new SL. |

### Session Timing

| Parameter | Type | Default | Description |
|---|---|---|---|
| `session_open` | `String` | *(per instrument)* | DAX: `"09:00"`, FTSE: `"08:00"`, NQ/DOW: `"09:30"`. |
| `session_timezone` | `String` | *(per instrument)* | DAX: `Europe/Berlin`, FTSE: `Europe/London`, NQ/DOW: `America/New_York`. |
| `signal_expiry_time` | `String` | `null` | Time after which unfilled orders cancel. `null` = no expiry. |
| `session_close` | `String` | *(per instrument)* | DAX: `"17:30"`, FTSE: `"16:30"`, NQ/DOW: `"16:00"`. |

### Backtest Scope

| Parameter | Type | Default | Description |
|---|---|---|---|
| `date_from` | `String` | `"2024-01-01"` | Start date (inclusive). |
| `date_to` | `String` | `"2025-12-31"` | End date (inclusive). |
| `initial_capital` | `f64` | `100000.0` | Starting capital for equity curve. |
| `position_size` | `f64` | `1.0` | Base position size in lots/contracts. |
| `point_value` | `f64` | *(per instrument)* | Cash value per point per lot. DAX CFD: `1.0`, DAX Future: `25.0`. |
| `commission_per_trade` | `f64` | `0.0` | Round-trip commission cost per trade. |
| `slippage_points` | `f64` | `0.5` | Simulated slippage per fill. |
| `exclude_dates` | `Vec<String>` | `[]` | Dates to exclude (holidays, data gaps). |

### Parameter Sweep

| Parameter | Type | Default | Description |
|---|---|---|---|
| `sweep_enabled` | `bool` | `false` | Enable parameter sweep mode. |
| `sweep_sl_fixed_points` | `Vec<f64>` | `[]` | Range of SL values. e.g., `[20, 30, 40, 50, 60]`. |
| `sweep_entry_offset_points` | `Vec<f64>` | `[]` | Range of entry offsets. e.g., `[0, 1, 2, 3, 5]`. |
| `sweep_trailing_stop_distance` | `Vec<f64>` | `[]` | Range of trail distances. |
| `sweep_add_every_points` | `Vec<f64>` | `[]` | Range of add intervals. |
| `sweep_signal_bar_index` | `Vec<u8>` | `[]` | e.g., `[1, 2, 3]` to compare candle positions. |
| `sweep_parallel_threads` | `u8` | `0` | Threads for sweep. `0` = all CPU cores. |

---

## Phase 0: Project Scaffold

**Goal:** Establish the monorepo structure with a Rust workspace, React dashboard, Docker stubs, and foundational config.

**Dependencies:** None.

### Workspace Cargo.toml

```toml
[workspace]
members = ["engine", "telegram"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
chrono-tz = "0.10"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
anyhow = "1"
thiserror = "2"
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "postgres", "chrono", "uuid", "json"] }
redis = { version = "0.27", features = ["tokio-comp", "aio"] }
reqwest = { version = "0.12", features = ["json"] }
rust_decimal = { version = "1", features = ["serde-with-str"] }
uuid = { version = "1", features = ["v4", "serde"] }
dotenvy = "0.15"
```

### Engine main.rs skeleton

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sr-engine", about = "School Run Strategy Engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Fetch { instrument: String },
    Backtest { config: String },
    Serve,
    Migrate,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Fetch { instrument } => todo!("Phase 1"),
        Commands::Backtest { config } => todo!("Phase 3"),
        Commands::Serve => todo!("Phase 5"),
        Commands::Migrate => todo!("Phase 4"),
    }
}
```

### Dashboard scaffold

```bash
npm create vite@latest dashboard -- --template react-ts
cd dashboard && npm install
npm install @tanstack/react-query recharts tailwindcss @tailwindcss/vite axios date-fns
```

### Acceptance Criteria

- `cargo check` succeeds from workspace root
- `cargo run --bin sr-engine -- --help` prints CLI help
- `cd dashboard && npm run dev` starts Vite on localhost:5173
- Git repository initialized with clean initial commit

---

## Phase 1: Data Pipeline

**Goal:** Retrieve 24+ months of 15-minute OHLCV candle data for all instruments, store as Parquet locally, and ingest into PostgreSQL.

**Dependencies:** Phase 0.

### Data Provider Selection

| Provider | Cost | Coverage | History | Verdict |
|---|---|---|---|---|
| **Twelve Data** | Free: 800/day. $79/mo growth. | DAX, FTSE, NQ, DOW | 10+ years 15-min | **Best overall** |
| **Polygon.io** | $29/mo starter | US indices only | 2+ years | US only, no European |
| **Alpha Vantage** | Free: 25/day | All four | 2 years intraday | Slow at free tier |
| **Yahoo Finance** | Free (unofficial) | All four | ~60 days intraday | **Disqualified** |

**Decision: Twelve Data.** Covers all four instruments at 15-min with deep history. Implement behind a `DataProvider` trait so providers can be swapped.

### Key Data Types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Instrument { Dax, Ftse, Nasdaq, Dow }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub instrument: Instrument,
    pub timestamp: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: i64,
}
```

### Storage

- **Parquet files:** `data/{instrument}/{YYYY-MM}.parquet` with Snappy compression
- **Postgres:** `candles` table partitioned by month, indexed on `(instrument_id, timestamp)`
- **Timezone:** All timestamps stored as UTC. Convert to CET/EST only at display boundaries. Use `chrono-tz` for DST-aware conversions.

### DST Handling

DAX opens at 09:00 CET year-round. CET = UTC+1 (winter), CEST = UTC+2 (summer). The signal bar time in UTC shifts with DST. Use `chrono_tz::Europe::Berlin` to compute correct UTC offset per date.

### Acceptance Criteria

- `sr-engine fetch DAX` fetches 24 months and writes Parquet files
- Candles upserted into Postgres with zero duplicates (idempotent)
- Signal bar for DAX appears as `08:15 UTC` (winter) or `07:15 UTC` (summer)
- Integration test: fetch 1 week, verify ~34 candles/day x 5 days

---

## Phase 2: Core Strategy Engine

**Goal:** Implement the School Run strategy logic as a pure, deterministic module with zero I/O. Candles in, signals and trades out.

**Dependencies:** Phase 1 (data model types).

### Key Structs

```rust
pub struct SignalBar {
    pub date: NaiveDate,
    pub instrument: Instrument,
    pub candle: Candle,
    pub buy_level: Decimal,     // candle.high + offset
    pub sell_level: Decimal,    // candle.low - offset
}

pub enum Direction { Long, Short }

pub struct PendingOrder {
    pub direction: Direction,
    pub trigger_price: Decimal,
    pub stop_loss: Decimal,
    pub expires_at: Option<DateTime<Utc>>,
}

pub struct Position {
    pub direction: Direction,
    pub entry_price: Decimal,
    pub entry_time: DateTime<Utc>,
    pub stop_loss: Decimal,
    pub size: Decimal,
    pub best_price: Decimal,    // For trailing stop
    pub adds: Vec<AddPosition>,
    pub status: PositionStatus,
}

pub struct Trade {
    pub instrument: Instrument,
    pub direction: Direction,
    pub entry_price: Decimal,
    pub entry_time: DateTime<Utc>,
    pub exit_price: Decimal,
    pub exit_time: DateTime<Utc>,
    pub stop_loss: Decimal,
    pub exit_reason: PositionStatus,
    pub pnl_points: Decimal,
    pub pnl_with_adds: Decimal,
    pub adds: Vec<AddResult>,
}
```

### Position Update Logic

Processing order within a single candle (critical for correctness):
1. Check if stop loss was hit (candle low for longs, candle high for shorts)
2. Update best_price (candle high for longs, candle low for shorts)
3. Check trailing stop against best_price
4. Check take profit
5. Check adding conditions
6. Check time-based exit

**Conservative assumption:** When both SL and a favorable price exist within the same candle, assume SL was hit first.

### Acceptance Criteria

- Unit test: `find_signal_bar` correctly identifies 09:15 CET candle for both winter and summer dates
- Unit test: `generate_orders` produces correct levels with 2-point offset for all three SL modes
- Unit test: `Position::update` handles SL hit, TP hit, and adding triggers
- All strategy logic testable with zero async, zero I/O

---

## Phase 3: Backtest Engine

**Goal:** High-performance backtesting engine producing trade logs, equity curves, and statistical summaries.

**Dependencies:** Phase 1, Phase 2.

### Core Algorithm

```rust
pub fn run_backtest(
    candles: &[Candle],
    instrument: Instrument,
    config: &StrategyConfig,
) -> BacktestResult {
    // 1. Group candles by trading date
    // 2. For each day:
    //    a. Find signal bar
    //    b. Generate pending orders (buy stop + sell stop)
    //    c. Iterate subsequent candles:
    //       - Check pending order fills
    //       - Update active positions (SL, TP, trailing, adds)
    //    d. Force-close open positions at EOD
    //    e. Record trades
    // 3. Compute drawdowns on equity curve
    // 4. Compute statistics
}
```

### Fill Simulation

- Buy stop at price P fills when `candle.high >= P`, at price P (or `candle.open` if it gaps above)
- When a candle could trigger both buy and sell: the direction closest to `candle.open` fills first

### BacktestStats

```rust
pub struct BacktestStats {
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub win_rate: f64,
    pub total_pnl: Decimal,
    pub avg_win: Decimal,
    pub avg_loss: Decimal,
    pub largest_win: Decimal,
    pub largest_loss: Decimal,
    pub profit_factor: f64,
    pub max_drawdown: Decimal,
    pub max_drawdown_pct: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub calmar_ratio: f64,
    pub max_consecutive_wins: u32,
    pub max_consecutive_losses: u32,
    pub avg_trade_duration_minutes: f64,
    pub long_trades: u32,
    pub short_trades: u32,
    pub long_pnl: Decimal,
    pub short_pnl: Decimal,
}
```

### Parameter Sweep

```rust
pub fn run_sweep(
    candles: &[Candle],
    instrument: Instrument,
    sweep: &SweepConfig,
) -> Vec<BacktestResult> {
    sweep.combinations()
        .par_iter()  // rayon parallel iteration
        .map(|config| run_backtest(candles, instrument, config))
        .collect()
}
```

### Performance Target

- 24 months x 4 instruments x ~34 candles/day x ~252 days = ~86,688 candles in under 100ms
- 100-combination sweep across 4 instruments in under 5 seconds

### Acceptance Criteria

- Known-answer test: hand-crafted 5-day dataset produces exactly expected trades and stats
- Edge cases pass: no signal bar (data gap), both sides triggered, order never filled, gap through entry, 3 adds with trailing
- Backtest benchmark under 100ms for single instrument via `criterion`

---

## Phase 4: Database + Cache Layer

**Goal:** PostgreSQL persistence and Valkey write-behind cache.

**Dependencies:** Phase 0, Phase 1, Phase 3.

### PostgreSQL Schema

```sql
-- instruments
CREATE TABLE instruments (
    id SMALLINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    symbol TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    open_time_local TEXT NOT NULL,
    close_time_local TEXT NOT NULL,
    timezone TEXT NOT NULL,
    tick_size NUMERIC(10,2) NOT NULL
);

-- candles (partitioned by month)
CREATE TABLE candles (
    instrument_id SMALLINT NOT NULL REFERENCES instruments(id),
    timestamp TIMESTAMPTZ NOT NULL,
    open NUMERIC(12,2) NOT NULL,
    high NUMERIC(12,2) NOT NULL,
    low NUMERIC(12,2) NOT NULL,
    close NUMERIC(12,2) NOT NULL,
    volume BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (instrument_id, timestamp)
) PARTITION BY RANGE (timestamp);

-- strategy_configs
CREATE TABLE strategy_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    params JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- backtest_runs
CREATE TABLE backtest_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_id UUID NOT NULL REFERENCES strategy_configs(id),
    instrument_id SMALLINT NOT NULL REFERENCES instruments(id),
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    total_trades INTEGER NOT NULL,
    stats JSONB NOT NULL,
    duration_ms INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- trades
CREATE TABLE trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    backtest_run_id UUID NOT NULL REFERENCES backtest_runs(id) ON DELETE CASCADE,
    instrument_id SMALLINT NOT NULL REFERENCES instruments(id),
    direction TEXT NOT NULL CHECK (direction IN ('long', 'short')),
    entry_price NUMERIC(12,2) NOT NULL,
    entry_time TIMESTAMPTZ NOT NULL,
    exit_price NUMERIC(12,2) NOT NULL,
    exit_time TIMESTAMPTZ NOT NULL,
    stop_loss NUMERIC(12,2) NOT NULL,
    exit_reason TEXT NOT NULL,
    pnl_points NUMERIC(12,2) NOT NULL,
    pnl_with_adds NUMERIC(12,2) NOT NULL,
    adds JSONB NOT NULL DEFAULT '[]',
    trade_date DATE NOT NULL
);

-- live_signals
CREATE TABLE live_signals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instrument_id SMALLINT NOT NULL REFERENCES instruments(id),
    signal_date DATE NOT NULL,
    signal_bar_high NUMERIC(12,2) NOT NULL,
    signal_bar_low NUMERIC(12,2) NOT NULL,
    buy_level NUMERIC(12,2) NOT NULL,
    sell_level NUMERIC(12,2) NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    fill_details JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (instrument_id, signal_date)
);

-- subscribers (for Telegram bot)
CREATE TABLE subscribers (
    id SERIAL PRIMARY KEY,
    chat_id BIGINT NOT NULL UNIQUE,
    username TEXT,
    subscribed_instruments TEXT[] NOT NULL DEFAULT '{}',
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### Valkey Key Patterns

| Key | Value | TTL | Purpose |
|---|---|---|---|
| `sr:signal:{instrument}:latest` | JSON SignalBar | 24h | Latest signal for quick reads |
| `sr:trades:{instrument}:today` | JSON Trade[] | 24h | Today's trades |
| `sr:backtest:{run_id}:progress` | JSON {progress, status} | 1h | Backtest progress tracking |
| `sr:backtest:{run_id}:result` | JSON BacktestResult | 7d | Cached backtest result |
| `sr:tracking:{instrument}:{date}` | JSON SignalState | 24h | Telegram bot state |

### Write-Behind Pipeline

Engine writes to Valkey via `CacheWriter`. A background Tokio task (`flush_worker`) drains a `mpsc` channel and batch-inserts into Postgres every 500ms or every 100 items. Uses `XREADGROUP` consumer groups for reliable delivery.

### Cache-First Reader

`CacheReader::get_backtest_result()` checks Valkey first, falls back to Postgres, backfills cache on miss.

### Acceptance Criteria

- `sqlx migrate run` executes all migrations on a fresh Postgres
- `candles` table is partitioned by month
- 100,000 candle inserts complete in under 2 seconds
- Write-behind flush delivers to Postgres within 500ms
- Cache-first reader returns cached result, falls back correctly

---

## Phase 5: Engine HTTP API

**Goal:** Expose all engine functionality via REST API on Axum.

**Dependencies:** Phase 2, Phase 3, Phase 4.

### Middleware Stack

```rust
Router::new()
    .nest("/api", api_routes())
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http())
    .layer(TimeoutLayer::new(Duration::from_secs(300)))
    .layer(CompressionLayer::new())
    .with_state(AppState { db_pool, valkey_pool, engine })
```

### Endpoints

#### Backtest

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/backtest/run` | Run a backtest, returns full result |
| `GET` | `/api/backtest/{id}` | Get backtest results |
| `GET` | `/api/backtest/{id}/trades` | Paginated trade list |
| `POST` | `/api/backtest/compare` | Run 2-4 configs, comparative stats |
| `GET` | `/api/backtest/history` | List past runs |

#### Strategy Configs

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/configs` | Save a config |
| `GET` | `/api/configs` | List saved configs |
| `GET` | `/api/configs/{id}` | Get config |
| `DELETE` | `/api/configs/{id}` | Delete config |

#### Data

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/data/candles` | Fetch candle data (query params: instrument, from, to) |
| `GET` | `/api/data/instruments` | List available instruments |
| `POST` | `/api/data/fetch` | Trigger data fetch from provider |

#### Signals

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/signals/today` | Today's signals across all instruments |
| `GET` | `/api/signals/{instrument}/latest` | Latest signal for instrument |

### Response Format

All responses JSON. Pagination via `?page=1&per_page=50` with wrapper:
```json
{
  "data": [ ... ],
  "pagination": { "page": 1, "per_page": 50, "total_items": 342, "total_pages": 7 }
}
```

Error format:
```json
{
  "error": { "code": "BACKTEST_NOT_FOUND", "message": "...", "details": null }
}
```

### Acceptance Criteria

- All endpoints return correct HTTP status codes
- `POST /api/backtest/run` executes a backtest and returns stats + equity curve
- Pagination works correctly on trade lists
- CORS enabled for dashboard access
- Error responses follow consistent format

---

## Phase 6: Dashboard (React + Vite + TypeScript)

**Goal:** Full-featured web dashboard for backtesting, charting, and signal monitoring.

**Dependencies:** Phase 5.

### Pages

1. **Backtest Page** (`/`) -- Parameter panel (left) + results (right). Instrument selector, date range, SL/exit/adding config. Run button. Stats card, equity curve, monthly heatmap, trade distribution histogram, trade log table.

2. **Chart Page** (`/chart`) -- Candlestick chart via `lightweight-charts`. Signal bar highlighted in blue. Entry/exit markers. Date navigation.

3. **Compare Page** (`/compare`) -- Select 2-4 configs, run comparison. Parameter diff view (differing values highlighted). Stats table (best value per row highlighted). Overlaid equity curves.

4. **Signals Page** (`/signals`) -- Today's signal bars per instrument. Status badges: WAITING (gray), PENDING (yellow), TRIGGERED (blue), STOPPED (red), CLOSED (green). Auto-refresh every 30s.

5. **History Page** (`/history`) -- Past backtest runs list. Load to view, delete old runs.

### State Management

- **Zustand** for client-side UI state (sidebar, theme) and current working config
- **TanStack Query** for all server state (backtest results, signals, configs, candles)

### Key Components

- `ParameterPanel` -- Dynamic form driven by config schema. Collapsible sections per parameter group. Conditional visibility (e.g., `fixed_points` input appears only when `sl_mode = "fixed"`)
- `EquityCurve` -- Recharts `<LineChart>` with gradient fill, drawdown shading
- `CandlestickChart` -- `lightweight-charts` wrapper with signal overlay and trade markers
- `StatsCard` -- 3x2 grid of stat tiles with color indicators
- `MonthlyHeatmap` -- Rows = years, columns = months, cell color = PnL magnitude
- `TradeTable` -- TanStack Table with sorting, filtering, pagination

### Acceptance Criteria

- All 5 pages render and navigate correctly
- Backtest can be configured, run, and results displayed
- Charts render correctly with real data
- Signal page auto-refreshes
- Responsive on desktop (mobile-responsive is Phase 9)

---

## Phase 7: Telegram Bot

**Goal:** Rust-based Telegram bot for signal notifications.

**Dependencies:** Phase 5.

### Commands

| Command | Description |
|---|---|
| `/start` | Register for notifications |
| `/signals` | Show today's signal levels |
| `/subscribe DAX,FTSE` | Choose instruments to follow |
| `/unsubscribe` | Stop notifications |
| `/status` | Current open signals and positions |

### Push Notifications

**Signal Bar Formed:**
```
DAX Signal Bar Formed
Time: 08:15 - 08:30 CET
High: 22,448 | Low: 22,390
Buy above: 22,450 | Sell below: 22,388
```

**Order Triggered:**
```
DAX LONG Triggered
Entry: 22,450 | SL: 22,390 | Risk: 60 pts
```

**Trade Closed:**
```
DAX LONG Closed
Entry: 22,450 -> Exit: 22,510
PnL: +60 points | Duration: 4h 13m
```

**Daily Summary:**
```
Daily Summary - 6 Mar 2026
DAX:  LONG  +60 pts
FTSE: SHORT -25 pts
Day Total: +35 points
```

### Architecture

- **Primary:** Valkey Pub/Sub -- engine publishes signal events, bot fans out to subscribers
- **Fallback:** Polling every 30s via `GET /api/signals/today`, diff against local state
- **State:** Subscriber preferences in Postgres. Tracking state in Valkey (sub-ms reads for `/status`)

### Acceptance Criteria

- Bot responds to all 5 commands
- Subscribed users receive push notifications on signal bar formation and order triggers
- Daily summary sent at market close
- Bot handles restarts gracefully (state in Postgres/Valkey, not in memory)

---

## Phase 8: Docker and Deployment

**Goal:** Dockerize all services with independent Dockerfiles.

**Dependencies:** All prior phases.

### Dockerfiles

**`engine/Dockerfile`** -- Multi-stage Rust build. Builder stage with dependency caching, slim Debian runtime.

**`dashboard/Dockerfile`** -- Node build stage compiles Vite app, nginx serves static assets. `nginx.conf` handles SPA routing and `/api` proxying to engine.

**`telegram/Dockerfile`** -- Multi-stage Rust build, same pattern as engine.

### Environment Variables

| Service | Variable | Description |
|---|---|---|
| Engine | `DATABASE_URL` | Postgres connection string |
| Engine | `VALKEY_URL` | Valkey/Redis connection string |
| Engine | `DATA_PROVIDER_API_KEY` | Twelve Data API key |
| Engine | `HOST` / `PORT` | Server bind address |
| Dashboard | `VITE_API_URL` | Engine API URL |
| Telegram | `TELEGRAM_BOT_TOKEN` | Bot token from BotFather |
| Telegram | `ENGINE_API_URL` | Engine API URL |
| Telegram | `DATABASE_URL` | Postgres connection string |
| Telegram | `VALKEY_URL` | Valkey/Redis connection string |

### Running

```bash
# Start infrastructure
docker run -d --name sr-postgres \
  -e POSTGRES_USER=sr -e POSTGRES_PASSWORD=sr_dev -e POSTGRES_DB=school_run \
  -p 5432:5432 postgres:16-alpine

docker run -d --name sr-valkey -p 6379:6379 valkey/valkey:8-alpine

# Build and run services independently
docker build -t sr-engine -f engine/Dockerfile .
docker run -d --name sr-engine --env-file engine/.env -p 3001:3001 sr-engine

docker build -t sr-dashboard -f dashboard/Dockerfile .
docker run -d --name sr-dashboard -p 3000:80 sr-dashboard

docker build -t sr-telegram -f telegram/Dockerfile .
docker run -d --name sr-telegram --env-file telegram/.env sr-telegram

# Initialize
docker exec sr-engine sr-engine migrate
docker exec sr-engine sr-engine fetch DAX --months 24
```

### Acceptance Criteria

- Each service builds independently as a Docker image
- Services can be started/stopped/updated independently
- Engine runs migrations on startup
- Dashboard loads and connects to engine API
- Telegram bot connects and responds to commands

---

## Phase 9: Polish and Future Enhancements

### Near-Term

- **Parameter sweep UI** -- "Optimize" tab with heatmap of PnL across parameter space (cap at 500 combinations)
- **Export** -- CSV/PDF download of backtest results
- **WebSocket** -- Replace polling on Signals page with `ws://host:3001/ws/signals`
- **Mobile-responsive** -- Tailwind breakpoints for all pages

### Medium-Term

- **Walk-forward analysis** -- In-sample optimize, out-of-sample validate, step through time
- **Monte Carlo simulation** -- Reshuffle trade sequence 10,000x, plot confidence intervals
- **Rate normalization** -- Normalize PnL by instrument volatility for cross-instrument comparison
- **Rolling Sharpe** -- Sharpe ratio over sliding window for regime analysis

### Long-Term

- **Broker integration (IBKR API)** -- Auto-place orders with safety controls (position limits, kill switches)
- **Multi-timeframe** -- Test signal bars on 5/10/30-min candles
- **Additional strategies** -- "American Sniper", "Soccer Mum", "KingFisher" as pluggable strategy modules

---

## Testing Strategy

Testing is not optional. Every phase includes testing as a first-class deliverable. Code without tests is not complete.

### Rust Testing Layers

| Layer | Tool | Location | What it covers |
|---|---|---|---|
| **Unit tests** | `cargo test` | `#[cfg(test)] mod tests` in each file | Individual functions, edge cases, pure logic |
| **Snapshot tests** | `insta` | Alongside unit tests | Complex output serialization (BacktestResult, Trade JSON) |
| **Integration tests** | `cargo test --features integration` | `engine/tests/` | Database queries, Valkey operations, API endpoints, full backtest pipeline |
| **Benchmarks** | `criterion` | `engine/benches/` | Backtest loop performance, parameter sweep throughput |
| **Property tests** | `proptest` | Alongside unit tests | Invariant checking (e.g., equity curve is consistent with trade PnL) |

### Dashboard Testing Layers

| Layer | Tool | What it covers |
|---|---|---|
| **Unit tests** | Vitest | Hooks, utility functions, store logic |
| **Component tests** | Vitest + Testing Library | Component rendering, user interactions |
| **API mocking** | MSW (Mock Service Worker) | API integration without a running engine |
| **Type checking** | `tsc --noEmit` | Compile-time type safety |

### Test Requirements Per Phase

| Phase | Required Tests |
|---|---|
| **Phase 1: Data** | Fetcher unit tests (API response parsing, rate limiting). Parquet round-trip test (write then read). Postgres upsert idempotency test. Timezone conversion tests (winter/summer DST). |
| **Phase 2: Strategy** | Signal bar detection (DAX winter, DAX summer, FTSE, NQ, DOW, holiday, data gap). Order generation (all 3 SL modes, both directions). Position update (SL hit, TP hit, trailing stop, adding triggers). Known-answer tests against hand-calculated examples from the PDF. |
| **Phase 3: Backtest** | Full backtest on hand-crafted 5-day dataset with predetermined outcomes. Edge cases: no signal bar day, both sides triggered, gap through entry, 3 adds with trailing. Stats verification: hand-calculated Sharpe, profit factor, max drawdown on small dataset. Benchmark: single instrument 24mo under 100ms. |
| **Phase 4: Database** | Migration execution on fresh Postgres. Candle bulk insert performance (100k rows < 2s). Write-behind flush timing (Valkey to Postgres < 500ms). Cache-first reader fallback behavior. |
| **Phase 5: API** | Every endpoint: correct status codes, response shapes, pagination, error format. Backtest run end-to-end (submit config, get result). CORS headers present. |
| **Phase 6: Dashboard** | Every component renders without crash. Parameter panel produces valid config JSON. Backtest flow: configure -> run -> view results. Chart renders with mock candle data. |
| **Phase 7: Telegram** | Command handlers respond correctly. Notification message formatting. Subscriber CRUD operations. |

### Test Helpers (engine/src/test_helpers.rs)

Shared test utilities compiled only under `#[cfg(test)]`:

```rust
pub fn make_candle(instrument: Instrument, ts: DateTime<Utc>, o: f64, h: f64, l: f64, c: f64) -> Candle
pub fn make_signal_bar(instrument: Instrument, date: NaiveDate, high: f64, low: f64) -> SignalBar
pub fn make_day_candles(instrument: Instrument, date: NaiveDate, bars: &[(f64, f64, f64, f64)]) -> Vec<Candle>
pub fn default_config() -> StrategyConfig
pub fn date(y: i32, m: u32, d: u32) -> NaiveDate
```

---

## CI/CD Pipeline

### GitHub Actions: `.github/workflows/ci.yml`

**On every push and PR:**

```yaml
jobs:
  rust-checks:
    - cargo fmt --check
    - cargo clippy -- -D warnings
    - cargo test

  dashboard-checks:
    - npm ci
    - npx prettier --check .
    - npx eslint .
    - npx tsc --noEmit
    - npx vitest run

  integration-tests:
    # Only on main branch or when label "run-integration" is added
    services: [postgres, valkey]
    - cargo test --features integration
```

**On merge to main:**
- Docker image builds for engine, dashboard, telegram
- Tag with commit SHA

---

## Documentation Plan

This is an open-source project. Documentation is as important as code.

### Documents to Produce

| Document | Location | Written During | Purpose |
|---|---|---|---|
| **README.md** | Root | Phase 0, updated each phase | Project intro, quick start, badges, screenshots |
| **BUILD.md** | Root | Pre-build (this file) | Internal build plan and architecture |
| **CLAUDE.md** | Root | Pre-build (done) | AI agent coding conventions |
| **CONTRIBUTING.md** | Root | Phase 8 | How to contribute, dev setup, PR process |
| **API Reference** | `docs/api.md` | Phase 5 | Full endpoint documentation with examples |
| **Strategy Guide** | `docs/strategy.md` | Phase 2 | How the School Run strategy works, with diagrams |
| **Data Guide** | `data/README.md` | Phase 1 | How to obtain and format historical data |
| **Deployment Guide** | `docs/deployment.md` | Phase 8 | How to run each service, env vars, troubleshooting |
| **Architecture Guide** | `docs/architecture.md` | Phase 4 | System design, data flow, tech decisions |
| **Inline Docs** | All source files | Every phase | `///` doc comments on all pub items |
| **CHANGELOG.md** | Root | Every phase | What changed, when |

### README.md Structure

```
# School Run Strategy

> Open-source trading strategy backtester based on Tom Hougaard's School Run Strategy

[Badges: CI status, license, Rust version, docs]

## What is this?
[2-paragraph overview with screenshot of dashboard]

## Quick Start
[5 commands to get running]

## Features
[Bullet list with checkmarks]

## Architecture
[Simplified diagram]

## Documentation
[Links to all docs]

## Contributing
[Link to CONTRIBUTING.md]

## License
MIT
```

---

## Team Execution Plan

Each build phase is executed by a dedicated **Team** (via `TeamCreate`). All teammates are **Opus 4.6 general-purpose agents**. No Sonnet. No Haiku. No sub-agents.

### Team Roles

Every phase team includes these roles:

| Role | Name Pattern | Responsibilities |
|---|---|---|
| **Lead (PM)** | Me (the user's agent) | Create team, create tasks, assign work, review all code, enforce CLAUDE.md, merge and shut down |
| **Builder(s)** | `{phase}-builder-1`, `{phase}-builder-2` | Write implementation code following CLAUDE.md conventions |
| **Tester** | `{phase}-tester` | Write unit tests, integration tests, snapshot tests. Verify coverage. Run clippy/fmt. |
| **Doc Writer** | `{phase}-docs` | Write doc comments, update docs/, update README sections |

### Phase Execution Template

For each phase:

1. **I create the team**: `TeamCreate` with name `sr-phase-{n}`
2. **I create tasks**: `TaskCreate` for each work item, with dependencies (`addBlockedBy`)
3. **I spawn teammates**: via `Agent` tool with `team_name` parameter, all `subagent_type: "general-purpose"`
4. **I assign tasks**: `TaskUpdate` with `owner` to give work to idle teammates
5. **Teammates work**: they implement, test, document, and message me when done
6. **I review**: read their code, check tests pass, check CLAUDE.md compliance, check docs
7. **I request fixes**: if anything is wrong, I message the teammate with feedback
8. **Quality gate**: all tests pass, clippy clean, docs present, I approve
9. **I shut down**: `SendMessage` shutdown_request to each teammate
10. **I clean up**: `TeamDelete` to remove the team
11. **I commit**: Git commit with conventional message

### Phase Team Breakdown

#### Phase 0: Scaffold (`sr-phase-0`)

| Teammate | Tasks |
|---|---|
| `scaffold-rust` | Init Cargo workspace, engine/Cargo.toml, telegram/Cargo.toml, main.rs stubs, lib.rs module declarations |
| `scaffold-frontend` | Init Vite + React + TS, install deps, tailwind setup, vite config with API proxy |
| `scaffold-infra` | Dockerfiles (stubs), .gitignore, .env.example, LICENSE (MIT), GitHub Actions CI stub |
| `scaffold-docs` | README.md initial version, data/README.md, docs/ directory structure |

#### Phase 1: Data Pipeline (`sr-phase-1`)

| Teammate | Tasks |
|---|---|
| `data-builder-1` | DataProvider trait, Twelve Data API client, rate limiting, pagination, retry logic |
| `data-builder-2` | Parquet storage (read/write), Postgres bulk insert, candle data model, timezone normalization |
| `data-tester` | Unit tests for API response parsing, Parquet round-trip, Postgres idempotency, DST conversion tests |
| `data-docs` | data/README.md (how to obtain data), doc comments on all pub items in data/ module |

#### Phase 2: Strategy Engine (`sr-phase-2`)

| Teammate | Tasks |
|---|---|
| `strategy-builder-1` | Signal bar detection, order generation, stop loss computation (all 3 modes) |
| `strategy-builder-2` | Position management, exit logic (all modes), adding to winners |
| `strategy-tester` | Unit tests for every function. Known-answer tests from PDF examples. DST edge cases. Both-sides-triggered scenario. Property tests for invariants. |
| `strategy-docs` | docs/strategy.md (strategy explanation with diagrams), doc comments on all pub items |

#### Phase 3: Backtest Engine (`sr-phase-3`)

| Teammate | Tasks |
|---|---|
| `backtest-builder-1` | Core backtest loop (day-by-day iteration, fill simulation, position tracking) |
| `backtest-builder-2` | Statistics computation, equity curve generation, parameter sweep with rayon |
| `backtest-tester` | Known-answer test (5-day hand-crafted dataset). Edge case tests. Stats verification. Criterion benchmarks (target: <100ms single instrument). |
| `backtest-docs` | Doc comments, update README with backtest usage examples |

#### Phase 4: Database + Cache (`sr-phase-4`)

| Teammate | Tasks |
|---|---|
| `db-builder-1` | PostgreSQL migrations (all 7 tables), SQLx query functions, connection pool setup |
| `db-builder-2` | Valkey integration (key patterns, write-behind pipeline, cache-first reader, pub/sub) |
| `db-tester` | Integration tests: migration execution, bulk insert performance, write-behind timing, cache fallback |
| `db-docs` | docs/architecture.md (data flow, Valkey patterns, schema docs) |

#### Phase 5: HTTP API (`sr-phase-5`)

| Teammate | Tasks |
|---|---|
| `api-builder-1` | Backtest endpoints (run, get, trades, compare, history) |
| `api-builder-2` | Config CRUD, data endpoints, signal endpoints, middleware (CORS, logging, errors) |
| `api-tester` | Integration tests for every endpoint. Status codes, response shapes, pagination, error format. End-to-end: submit backtest config -> get results. |
| `api-docs` | docs/api.md (full endpoint reference with curl examples) |

#### Phase 6: Dashboard (`sr-phase-6`)

| Teammate | Tasks |
|---|---|
| `dash-builder-1` | Layout (AppShell, Sidebar), BacktestPage (ParameterPanel, StatsCard), HistoryPage |
| `dash-builder-2` | ChartPage (CandlestickChart with lightweight-charts, SignalOverlay), ComparePage, SignalsPage |
| `dash-builder-3` | API client layer, TanStack Query hooks, Zustand stores, TypeScript types |
| `dash-tester` | Vitest + Testing Library tests for all components. MSW mocks for API. Type checking. |
| `dash-docs` | Component JSDoc comments, update README with screenshots |

#### Phase 7: Telegram Bot (`sr-phase-7`)

| Teammate | Tasks |
|---|---|
| `telegram-builder` | Bot setup (teloxide), command handlers (/start, /signals, /subscribe, /status), Valkey pub/sub listener, push notifications |
| `telegram-tester` | Unit tests for command handlers, notification formatting, subscriber CRUD |
| `telegram-docs` | Bot usage documentation in README, doc comments |

#### Phase 8: Docker + Deployment (`sr-phase-8`)

| Teammate | Tasks |
|---|---|
| `docker-builder` | Multi-stage Dockerfiles (engine, dashboard, telegram), nginx.conf, env var documentation |
| `docker-tester` | Verify each image builds, verify services start and connect, verify migrations run |
| `docker-docs` | docs/deployment.md, CONTRIBUTING.md, finalize README |

### Quality Review Checklist (PM uses this for every phase)

```
[ ] All tasks marked complete by teammates
[ ] cargo fmt --check passes (Rust)
[ ] cargo clippy -- -D warnings passes (Rust)
[ ] cargo test passes with zero failures
[ ] prettier/eslint pass (dashboard)
[ ] vitest passes (dashboard)
[ ] Every pub function has a unit test
[ ] Every pub item has doc comments
[ ] No .unwrap() in production code
[ ] No any types in TypeScript
[ ] Documentation updated (docs/, README)
[ ] CHANGELOG.md entry added
[ ] Git commit with conventional message
```
