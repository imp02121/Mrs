//! Shared test helpers and factory functions.
//!
//! This module is only compiled in test builds. It provides convenient
//! constructors for domain types used across unit tests.

// TODO: `make_candle(timestamp, open, high, low, close)` -- factory for test candles
// TODO: `make_day_candles(instrument, date, signal_bar_ohlc, post_bar_candles)` -- builds a full day
// TODO: `default_config()` -- returns a reasonable default `StrategyConfig`
// TODO: `date(y, m, d)` -- shorthand for `NaiveDate::from_ymd_opt(y, m, d).unwrap()`
