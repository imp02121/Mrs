# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
