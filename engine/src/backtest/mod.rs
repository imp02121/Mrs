//! Backtest engine: core loop, parameter handling, statistics, and reporting.
//!
//! - [`engine`] -- Core backtest loop via [`run_backtest`]
//! - [`result`] -- [`BacktestResult`], [`EquityPoint`], [`DailyPnl`]
//! - [`stats`] -- [`BacktestStats`] computation (Sharpe, drawdown, win rate, etc.)
//! - [`sweep`] -- [`SweepConfig`] and parallel parameter sweep via [`run_sweep`]
//! - [`report`] -- Serialization helpers and display formatting

pub mod engine;
pub mod report;
pub mod result;
pub mod stats;
pub mod sweep;

pub use engine::run_backtest;
pub use report::{BacktestSummary, to_json, to_json_compact};
pub use result::{BacktestResult, DailyPnl, EquityPoint};
pub use stats::{BacktestStats, compute_stats};
pub use sweep::{SweepConfig, SweepResult, best_by, run_sweep};

#[cfg(test)]
mod backtest_tests;
