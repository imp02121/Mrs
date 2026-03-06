//! OHLCV candle representation and related date-range utilities.
//!
//! [`Candle`] is the fundamental market-data building block for the entire
//! engine: strategy logic, backtesting, and data ingestion all operate on
//! candles. [`DateRange`] is a simple inclusive date pair used when requesting
//! or filtering historical data.

use std::fmt;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::instrument::Instrument;

/// A single OHLCV (Open-High-Low-Close-Volume) candle.
///
/// All timestamps are bar-open times expressed in UTC.  Financial values
/// (open, high, low, close) use [`Decimal`] for exact arithmetic; volume
/// is a signed 64-bit integer to accommodate provider APIs that may return
/// negative adjustments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    /// The trading instrument this candle belongs to.
    pub instrument: Instrument,
    /// Bar open time in UTC.
    pub timestamp: DateTime<Utc>,
    /// Opening price of the bar.
    pub open: Decimal,
    /// Highest price reached during the bar.
    pub high: Decimal,
    /// Lowest price reached during the bar.
    pub low: Decimal,
    /// Closing price of the bar.
    pub close: Decimal,
    /// Number of contracts/shares traded during the bar.
    pub volume: i64,
}

impl Candle {
    /// The bar's full range (high - low).
    #[must_use]
    pub fn range(&self) -> Decimal {
        self.high - self.low
    }

    /// The candle body size (absolute difference between open and close).
    #[must_use]
    pub fn body(&self) -> Decimal {
        (self.close - self.open).abs()
    }

    /// Returns `true` if the candle closed above its open (bullish).
    #[must_use]
    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    /// Returns `true` if the candle closed below its open (bearish).
    #[must_use]
    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }
}

impl fmt::Display for Candle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} O={} H={} L={} C={} V={}",
            self.instrument.ticker(),
            self.timestamp.format("%Y-%m-%d %H:%M"),
            self.open,
            self.high,
            self.low,
            self.close,
            self.volume,
        )
    }
}

/// An inclusive date range used to request or filter historical candle data.
///
/// Both `start` and `end` are inclusive.  `start` must be <= `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateRange {
    /// First date (inclusive).
    pub start: NaiveDate,
    /// Last date (inclusive).
    pub end: NaiveDate,
}

/// Error returned when constructing an invalid [`DateRange`].
#[derive(Debug, thiserror::Error)]
#[error("invalid date range: start ({start}) is after end ({end})")]
pub struct DateRangeError {
    /// The start date that was after end.
    pub start: NaiveDate,
    /// The end date that was before start.
    pub end: NaiveDate,
}

impl DateRange {
    /// Create a new date range, validating that `start <= end`.
    ///
    /// # Errors
    ///
    /// Returns [`DateRangeError`] if `start` is after `end`.
    pub fn new(start: NaiveDate, end: NaiveDate) -> Result<Self, DateRangeError> {
        if start > end {
            return Err(DateRangeError { start, end });
        }
        Ok(Self { start, end })
    }

    /// Number of calendar days in the range (inclusive on both ends).
    #[must_use]
    pub fn days(&self) -> i64 {
        (self.end - self.start).num_days() + 1
    }

    /// Returns `true` if `date` falls within this range (inclusive).
    #[must_use]
    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end
    }

    /// Iterate over every date in the range.
    pub fn iter(&self) -> impl Iterator<Item = NaiveDate> {
        let start = self.start;
        let end = self.end;
        let mut current = start;
        std::iter::from_fn(move || {
            if current > end {
                return None;
            }
            let d = current;
            current = current.succ_opt().unwrap_or(current);
            Some(d)
        })
    }
}

impl fmt::Display for DateRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} to {}", self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn sample_candle() -> Candle {
        Candle {
            instrument: Instrument::Dax,
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            open: d("16000.50"),
            high: d("16050.00"),
            low: d("15980.25"),
            close: d("16030.75"),
            volume: 12345,
        }
    }

    #[test]
    fn test_candle_range() {
        let c = sample_candle();
        assert_eq!(c.range(), d("69.75")); // 16050.00 - 15980.25
    }

    #[test]
    fn test_candle_body() {
        let c = sample_candle();
        assert_eq!(c.body(), d("30.25")); // |16030.75 - 16000.50|
    }

    #[test]
    fn test_candle_is_bullish() {
        let c = sample_candle();
        assert!(c.is_bullish()); // close > open
        assert!(!c.is_bearish());
    }

    #[test]
    fn test_candle_is_bearish() {
        let mut c = sample_candle();
        c.close = d("15990.00");
        assert!(c.is_bearish());
        assert!(!c.is_bullish());
    }

    #[test]
    fn test_candle_display() {
        let c = sample_candle();
        let s = format!("{c}");
        assert!(s.contains("DAX"));
        assert!(s.contains("16000.50"));
    }

    #[test]
    fn test_candle_serde_roundtrip() {
        let c = sample_candle();
        let json = serde_json::to_string(&c).unwrap();
        let parsed: Candle = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.open, c.open);
        assert_eq!(parsed.high, c.high);
        assert_eq!(parsed.instrument, c.instrument);
    }

    // -- DateRange tests --

    #[test]
    fn test_date_range_new_valid() {
        let dr = DateRange::new(date(2024, 1, 1), date(2024, 12, 31)).unwrap();
        assert_eq!(dr.start, date(2024, 1, 1));
        assert_eq!(dr.end, date(2024, 12, 31));
    }

    #[test]
    fn test_date_range_new_same_day() {
        let dr = DateRange::new(date(2024, 6, 15), date(2024, 6, 15)).unwrap();
        assert_eq!(dr.days(), 1);
    }

    #[test]
    fn test_date_range_new_invalid() {
        let err = DateRange::new(date(2024, 12, 31), date(2024, 1, 1)).unwrap_err();
        assert!(err.to_string().contains("invalid date range"));
    }

    #[test]
    fn test_date_range_days() {
        let dr = DateRange::new(date(2024, 1, 1), date(2024, 1, 10)).unwrap();
        assert_eq!(dr.days(), 10);
    }

    #[test]
    fn test_date_range_contains() {
        let dr = DateRange::new(date(2024, 1, 1), date(2024, 1, 31)).unwrap();
        assert!(dr.contains(date(2024, 1, 1))); // start inclusive
        assert!(dr.contains(date(2024, 1, 31))); // end inclusive
        assert!(dr.contains(date(2024, 1, 15)));
        assert!(!dr.contains(date(2023, 12, 31)));
        assert!(!dr.contains(date(2024, 2, 1)));
    }

    #[test]
    fn test_date_range_iter() {
        let dr = DateRange::new(date(2024, 1, 1), date(2024, 1, 5)).unwrap();
        let dates: Vec<_> = dr.iter().collect();
        assert_eq!(dates.len(), 5);
        assert_eq!(dates[0], date(2024, 1, 1));
        assert_eq!(dates[4], date(2024, 1, 5));
    }

    #[test]
    fn test_date_range_display() {
        let dr = DateRange::new(date(2024, 1, 1), date(2024, 12, 31)).unwrap();
        assert_eq!(format!("{dr}"), "2024-01-01 to 2024-12-31");
    }

    #[test]
    fn test_date_range_serde_roundtrip() {
        let dr = DateRange::new(date(2024, 1, 1), date(2024, 12, 31)).unwrap();
        let json = serde_json::to_string(&dr).unwrap();
        let parsed: DateRange = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, dr);
    }

    // -- Additional candle edge case tests --

    #[test]
    fn test_candle_doji_body_is_zero() {
        let mut c = sample_candle();
        c.close = c.open;
        assert_eq!(c.body(), Decimal::ZERO);
        assert!(!c.is_bullish());
        assert!(!c.is_bearish());
    }

    #[test]
    fn test_candle_zero_range() {
        let c = Candle {
            instrument: Instrument::Ftse,
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            open: d("100.00"),
            high: d("100.00"),
            low: d("100.00"),
            close: d("100.00"),
            volume: 0,
        };
        assert_eq!(c.range(), Decimal::ZERO);
        assert_eq!(c.body(), Decimal::ZERO);
    }

    #[test]
    fn test_candle_display_format_complete() {
        let c = Candle {
            instrument: Instrument::Nasdaq,
            timestamp: chrono::NaiveDate::from_ymd_opt(2024, 7, 15)
                .unwrap()
                .and_hms_opt(13, 45, 0)
                .unwrap()
                .and_utc(),
            open: d("15000.00"),
            high: d("15100.00"),
            low: d("14900.00"),
            close: d("15050.00"),
            volume: 50000,
        };
        let s = format!("{c}");
        assert!(s.contains("IXIC"));
        assert!(s.contains("2024-07-15 13:45"));
        assert!(s.contains("O=15000.00"));
        assert!(s.contains("H=15100.00"));
        assert!(s.contains("L=14900.00"));
        assert!(s.contains("C=15050.00"));
        assert!(s.contains("V=50000"));
    }

    #[test]
    fn test_candle_serde_all_instruments() {
        for instrument in [
            Instrument::Dax,
            Instrument::Ftse,
            Instrument::Nasdaq,
            Instrument::Dow,
        ] {
            let c = Candle {
                instrument,
                timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
                open: d("100.0"),
                high: d("110.0"),
                low: d("90.0"),
                close: d("105.0"),
                volume: 100,
            };
            let json = serde_json::to_string(&c).unwrap();
            let parsed: Candle = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.instrument, instrument);
        }
    }

    // -- Additional DateRange edge case tests --

    #[test]
    fn test_date_range_iter_single_day() {
        let dr = DateRange::new(date(2024, 6, 15), date(2024, 6, 15)).unwrap();
        let dates: Vec<_> = dr.iter().collect();
        assert_eq!(dates, vec![date(2024, 6, 15)]);
    }

    #[test]
    fn test_date_range_days_leap_year() {
        // 2024 is a leap year, Feb has 29 days.
        let dr = DateRange::new(date(2024, 2, 1), date(2024, 2, 29)).unwrap();
        assert_eq!(dr.days(), 29);
    }

    #[test]
    fn test_date_range_contains_boundaries() {
        let dr = DateRange::new(date(2024, 3, 10), date(2024, 3, 10)).unwrap();
        assert!(dr.contains(date(2024, 3, 10)));
        assert!(!dr.contains(date(2024, 3, 9)));
        assert!(!dr.contains(date(2024, 3, 11)));
    }

    #[test]
    fn test_date_range_error_message_contains_dates() {
        let err = DateRange::new(date(2025, 1, 1), date(2024, 1, 1)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("2025-01-01"));
        assert!(msg.contains("2024-01-01"));
    }
}
