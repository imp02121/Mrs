//! Shared test helpers and factory functions.
//!
//! This module is only compiled in test builds. It provides convenient
//! constructors for domain types used across unit tests.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::models::{Candle, Instrument};
use crate::strategy::config::StrategyConfig;
use crate::strategy::signal::SignalBar;

/// Shorthand for constructing a [`NaiveDate`].
///
/// # Panics
///
/// Panics if the year/month/day combination is invalid.
pub fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("invalid date in test helper")
}

/// Shorthand for constructing a [`DateTime<Utc>`].
///
/// # Panics
///
/// Panics if the date/time combination is invalid.
pub fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
    NaiveDate::from_ymd_opt(y, m, d)
        .expect("invalid date in test helper")
        .and_time(NaiveTime::from_hms_opt(h, min, 0).expect("invalid time in test helper"))
        .and_utc()
}

/// Construct a single [`Candle`] from an instrument, a timestamp string, and OHLC values.
///
/// The timestamp string must be in `"YYYY-MM-DD HH:MM"` format (UTC).
/// Volume defaults to 1000.
///
/// # Panics
///
/// Panics on invalid timestamp or decimal strings.
pub fn make_candle(
    instrument: Instrument,
    timestamp_str: &str,
    open: &str,
    high: &str,
    low: &str,
    close: &str,
) -> Candle {
    let naive = chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M")
        .expect("invalid timestamp in make_candle");
    let timestamp = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);

    Candle {
        instrument,
        timestamp,
        open: Decimal::from_str(open).expect("invalid open in make_candle"),
        high: Decimal::from_str(high).expect("invalid high in make_candle"),
        low: Decimal::from_str(low).expect("invalid low in make_candle"),
        close: Decimal::from_str(close).expect("invalid close in make_candle"),
        volume: 1000,
    }
}

/// Build a full day of 15-minute candles for an instrument starting at the signal bar.
///
/// `bars` is a slice of `(open, high, low, close)` tuples as `f64`.
/// The first bar starts at the instrument's signal bar UTC time for the given date.
/// Subsequent bars are spaced 15 minutes apart.
///
/// # Panics
///
/// Panics if the signal bar UTC time cannot be computed for the given date.
pub fn make_day_candles(
    instrument: Instrument,
    day: NaiveDate,
    bars: &[(f64, f64, f64, f64)],
) -> Vec<Candle> {
    let start = instrument
        .signal_bar_start_utc(day)
        .expect("signal_bar_start_utc returned None in test");

    bars.iter()
        .enumerate()
        .map(|(i, &(o, h, l, c))| {
            let ts = start + chrono::Duration::minutes(15 * i as i64);
            Candle {
                instrument,
                timestamp: ts,
                open: Decimal::try_from(o).expect("invalid open f64"),
                high: Decimal::try_from(h).expect("invalid high f64"),
                low: Decimal::try_from(l).expect("invalid low f64"),
                close: Decimal::try_from(c).expect("invalid close f64"),
                volume: 1000 + i as i64,
            }
        })
        .collect()
}

/// Shorthand for parsing a [`Decimal`] from a string.
///
/// # Panics
///
/// Panics if the string is not a valid decimal.
pub fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("invalid decimal in test helper")
}

/// Returns a default [`StrategyConfig`] suitable for most tests.
///
/// Matches the original School Run parameters: DAX, 2nd candle,
/// 2-point offset, fixed 40-point stop, end-of-day exit.
pub fn default_config() -> StrategyConfig {
    StrategyConfig::default()
}

/// Construct a [`SignalBar`] from an instrument, date, and OHLC values.
///
/// Creates a candle at the correct signal bar UTC time for the given
/// instrument and date, then wraps it in a `SignalBar` with buy/sell
/// levels computed from the config's entry offset.
///
/// # Panics
///
/// Panics if the signal bar UTC time cannot be computed.
pub fn make_signal_bar(
    instrument: Instrument,
    day: NaiveDate,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    config: &StrategyConfig,
) -> SignalBar {
    let candles = make_day_candles(instrument, day, &[(open, high, low, close)]);
    let candle = candles
        .into_iter()
        .next()
        .expect("make_day_candles produced no candles");
    SignalBar {
        date: day,
        instrument,
        buy_level: candle.high + config.entry_offset_points,
        sell_level: candle.low - config.entry_offset_points,
        candle,
    }
}
