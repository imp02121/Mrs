# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Backtest engine (Phase 3)
  - Core backtest loop (`run_backtest`) with day-by-day iteration over candles applying the School Run Strategy
  - Candle grouping by trading date using instrument's exchange timezone
  - Signal bar detection, order generation, fill simulation, and position tracking per session
  - Signal expiry: cancel unfilled orders after a configurable time
  - Force-close open positions at end of day
  - Deterministic: same candles + config always produce the same result
  - `BacktestResult` struct with trades, equity curve, daily PnL, and computed statistics
  - `EquityPoint` and `DailyPnl` types for equity curve and daily profit tracking
  - `BacktestStats` with comprehensive metrics: win rate, profit factor, Sharpe ratio, Sortino ratio, Calmar ratio, max drawdown (absolute and percentage), consecutive win/loss streaks, average trade duration, and long/short breakdown
  - Edge case handling: zero trades, all wins, all losses, single trade, zero standard deviation
  - Parameter sweep (`SweepConfig`, `run_sweep`) with Cartesian product of configurable axes: stop loss distance, entry offset, trailing stop distance, add-to-winners interval, signal bar index
  - Parallel sweep execution using Rayon with configurable thread count
  - `best_by` helper for finding the best sweep result by any metric
  - Report module with JSON serialization (`to_json`, `to_json_compact`) and `BacktestSummary` display formatting
  - `Display` impl for `BacktestStats` for human-readable output
  - Comprehensive unit tests for all backtest modules

- Core strategy engine (Phase 2)
  - `StrategyConfig` struct with all configurable parameters (signal detection, stop loss, exit, adding to winners, session timing, backtest scope) using `Decimal` for financial values
  - `Direction`, `StopLossMode`, `ExitMode`, `PositionStatus` enums with serde and Display
  - Signal bar detection (`find_signal_bar`) with DST-aware UTC timestamp matching
  - `SignalBar` struct with pre-computed buy/sell stop levels
  - Order generation (`generate_orders`) with three stop loss modes: SignalBarExtreme, FixedPoints, Midpoint
  - Index-level-proportional stop loss scaling (`sl_scale_with_index`)
  - Fill simulation (`check_fill`, `determine_fill_order`) with gap handling, slippage, and both-sides-triggered priority
  - `Position` struct with 6-step per-candle update logic (SL, best price, trailing, TP, adds, time exit)
  - Conservative assumption: stop loss checked before any favorable exit on same candle
  - Adding to winners (`check_add_trigger`) with configurable intervals, max additions, size multiplier, and stop tightening
  - `Trade` struct with full PnL computation including add-on positions, commission, and slippage
  - Comprehensive unit tests for all strategy modules (all four instruments, DST transitions, edge cases)
- Strategy documentation (`docs/strategy.md`) with worked examples, parameter reference, and processing order explanation

- Core data models (Phase 1)
  - `Candle` struct with OHLCV fields, `Decimal` precision, and display formatting
  - `Instrument` enum (DAX, FTSE, Nasdaq, Dow) with DST-aware signal bar UTC conversion
  - `DateRange` inclusive date pair with validation, iteration, and containment checks
  - `ParseInstrumentError` and `DateRangeError` for robust error handling
- Twelve Data API integration for 15-minute OHLCV data
  - `DataProvider` trait for provider-agnostic data fetching
  - `TwelveDataProvider` with automatic pagination (5000-row chunks of 145 days)
  - Rate limiting via semaphore with configurable inter-request delay
  - Exponential backoff retry on transient errors (HTTP 429, 5xx)
  - OHLCV validation (high >= low, open/close within range)
- Parquet file storage for local candle data
  - `ParquetStore` with per-instrument, per-month file partitioning
  - Snappy compression, Arrow-based serialization
  - Read with date range filtering across multiple monthly files
- PostgreSQL bulk insert for candle data
  - `PostgresStore` with `INSERT ... ON CONFLICT` idempotent upserts
  - Batched writes (1000 rows per INSERT) to stay within Postgres parameter limits
  - Query by instrument and date range, latest timestamp lookup
- `DataFetcher` orchestrator for full backfill and incremental fetch modes
  - Deduplication by timestamp before storage
  - Writes to both Parquet and Postgres in a single operation
- DST-aware timezone handling for all four instruments
  - Uses `chrono-tz` IANA timezone database for correct UTC offset per date
  - Tested across spring/autumn DST transitions for all regions
- `DataError` enum with variants for I/O, Parquet, Arrow, database, API, rate limit, and validation errors

- Initial project scaffold (Phase 0)
  - Rust workspace with `engine` and `telegram` crates
  - React + Vite + TypeScript dashboard scaffold
  - Dockerfiles for engine, dashboard, and telegram services
  - PostgreSQL migration stubs
  - Project documentation (README, data guide, doc stubs)
  - CI workflow stub (GitHub Actions)
  - MIT license
