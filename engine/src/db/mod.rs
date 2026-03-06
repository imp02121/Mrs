//! Database and cache integration layer.
//!
//! This module provides three layers for data persistence and caching:
//!
//! ## PostgreSQL Queries
//!
//! Each table has a dedicated submodule with typed row structs (`sqlx::FromRow`)
//! and async query functions. Modules: [`instruments`], [`candles`], [`configs`],
//! [`backtests`], [`trades`], [`signals`], [`subscribers`].
//!
//! ## Valkey Cache
//!
//! [`ValkeyCache`] provides JSON-serialized get/set/delete against a Valkey
//! (Redis-compatible) instance. Domain helpers cache backtest results (7d TTL),
//! signals (24h TTL), and progress (1h TTL) using the `sr:` key prefix.
//!
//! ## Write-Behind Pipeline
//!
//! [`WriteBehindWorker`] runs a background Tokio task that drains [`WriteOp`]
//! items from an mpsc channel (via [`WriteBehindSender`]) and flushes them to
//! Postgres in batches (every 500ms or 100 items). This decouples hot-path
//! writes from database latency.
//!
//! [`CacheReader`] implements cache-aside reads: check Valkey first, fall back
//! to Postgres on miss, and backfill the cache for subsequent reads.

pub mod backtests;
pub mod cache;
pub mod cache_error;
pub mod cache_reader;
pub mod candles;
pub mod configs;
pub mod error;
pub mod instruments;
pub mod signals;
pub mod subscribers;
pub mod trades;
pub mod write_behind;

#[cfg(test)]
mod migration_tests;

pub use backtests::{BacktestRunRow, InsertBacktestRun};
pub use cache::ValkeyCache;
pub use cache_error::CacheError;
pub use cache_reader::{CacheReader, ReaderError};
pub use candles::CandleRow;
pub use configs::ConfigRow;
pub use error::DbError;
pub use instruments::InstrumentRow;
pub use signals::{SignalRow, UpsertSignal};
pub use subscribers::SubscriberRow;
pub use trades::{InsertTrade, TradeRow};
pub use write_behind::{WriteBehindSender, WriteBehindWorker, WriteOp};
