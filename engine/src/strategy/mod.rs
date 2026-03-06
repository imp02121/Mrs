//! Strategy logic: signal bar detection, order placement, stop loss, exits, and adding to winners.
//!
//! This module contains the core School Run Strategy implementation:
//!
//! - [`config`] -- [`StrategyConfig`] with all configurable parameters
//! - [`types`] -- Shared enums: [`Direction`], [`StopLossMode`], [`ExitMode`], [`PositionStatus`]
//! - [`signal`] -- Signal bar detection via [`find_signal_bar`]
//! - [`order`] -- Order generation and stop loss computation via [`generate_orders`]
//! - [`fill`] -- Fill simulation via [`check_fill`] and [`determine_fill_order`]
//! - [`position`] -- Position tracking and per-candle update logic
//! - [`add_to_winners`] -- Adding to winning positions at configured intervals
//! - [`trade`] -- Completed trade records with PnL computation

pub mod add_to_winners;
pub mod config;
pub mod fill;
#[cfg(test)]
mod integration_tests;
pub mod order;
pub mod position;
pub mod signal;
#[cfg(test)]
mod strategy_tests;
pub mod trade;
pub mod types;

pub use config::StrategyConfig;
pub use fill::{FillResult, check_fill, determine_fill_order};
pub use order::{PendingOrder, generate_orders};
pub use position::{AddPosition, ExitResult, Position};
pub use signal::{SignalBar, find_signal_bar};
pub use trade::{AddResult, Trade};
pub use types::{Direction, ExitMode, PositionStatus, StopLossMode};
