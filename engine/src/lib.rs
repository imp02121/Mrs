//! Core library for the School Run Strategy engine.
//!
//! Provides strategy logic, backtesting, HTTP API, database access,
//! data ingestion, and shared domain models.

pub mod api;
pub mod backtest;
pub mod data;
pub mod db;
pub mod models;
pub mod strategy;

#[cfg(test)]
pub mod test_helpers;
