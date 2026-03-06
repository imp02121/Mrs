//! Signal bar detection for the School Run Strategy.
//!
//! The signal bar is the Nth 15-minute candle after market open (default: 2nd).
//! This module identifies it from a slice of candles and computes the
//! buy/sell stop levels based on entry offset.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::{Candle, Instrument};

use super::config::StrategyConfig;

/// A detected signal bar with pre-computed entry levels.
///
/// The buy and sell levels are computed as:
/// - `buy_level = candle.high + entry_offset_points`
/// - `sell_level = candle.low - entry_offset_points`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalBar {
    /// The trading date this signal bar belongs to.
    pub date: NaiveDate,
    /// The instrument this signal bar was detected for.
    pub instrument: Instrument,
    /// The actual candle that is the signal bar.
    pub candle: Candle,
    /// Buy stop level: signal bar high + entry offset.
    pub buy_level: Decimal,
    /// Sell stop level: signal bar low - entry offset.
    pub sell_level: Decimal,
}

/// Identifies the signal bar for a given trading day and instrument.
///
/// The signal bar is the Nth 15-minute candle after market open, where N is
/// determined by `config.signal_bar_index`. For DAX with default settings,
/// this is the 09:15-09:30 CET candle (index 2, the 2nd candle after open).
///
/// The function uses [`Instrument::signal_bar_start_utc`] to find the
/// candle whose timestamp matches the expected signal bar start time in UTC.
///
/// Returns `None` if:
/// - The candle slice is empty
/// - No candle matches the expected signal bar timestamp
/// - The signal bar UTC time cannot be computed (DST edge case)
///
/// # Arguments
///
/// * `candles` - Day's candles for the instrument, sorted by timestamp
/// * `instrument` - The trading instrument (determines market open time)
/// * `date` - The trading date
/// * `config` - Strategy configuration (determines bar index and offset)
#[must_use]
pub fn find_signal_bar(
    candles: &[Candle],
    instrument: Instrument,
    date: NaiveDate,
    config: &StrategyConfig,
) -> Option<SignalBar> {
    let signal_bar_utc = instrument.signal_bar_start_utc(date)?;

    let candle = candles.iter().find(|c| c.timestamp == signal_bar_utc)?;

    Some(SignalBar {
        date,
        instrument,
        candle: candle.clone(),
        buy_level: candle.high + config.entry_offset_points,
        sell_level: candle.low - config.entry_offset_points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{date, make_candle, make_day_candles};
    use rust_decimal_macros::dec;

    fn default_config() -> StrategyConfig {
        StrategyConfig::default()
    }

    // -- DAX tests --

    #[test]
    fn test_signal_bar_found_for_dax_winter() {
        // 2024-01-15: CET (UTC+1), signal bar at 09:15 CET = 08:15 UTC
        let d = date(2024, 1, 15);
        let candles = make_day_candles(
            Instrument::Dax,
            d,
            &[
                (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
                (16030.0, 16060.0, 16010.0, 16045.0),
            ],
        );
        let config = default_config();
        let result = find_signal_bar(&candles, Instrument::Dax, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.date, d);
        assert_eq!(bar.instrument, Instrument::Dax);
        assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "08:15");
        assert_eq!(bar.buy_level, dec!(16052)); // 16050 + 2
        assert_eq!(bar.sell_level, dec!(15978)); // 15980 - 2
    }

    #[test]
    fn test_signal_bar_found_for_dax_summer() {
        // 2024-07-15: CEST (UTC+2), signal bar at 09:15 CEST = 07:15 UTC
        let d = date(2024, 7, 15);
        let candles = make_day_candles(
            Instrument::Dax,
            d,
            &[
                (18000.0, 18100.0, 17950.0, 18050.0),
                (18050.0, 18120.0, 18020.0, 18080.0),
            ],
        );
        let config = default_config();
        let result = find_signal_bar(&candles, Instrument::Dax, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "07:15");
        assert_eq!(bar.buy_level, dec!(18102)); // 18100 + 2
        assert_eq!(bar.sell_level, dec!(17948)); // 17950 - 2
    }

    // -- FTSE tests --

    #[test]
    fn test_signal_bar_found_for_ftse_winter() {
        // 2024-01-15: GMT (UTC+0), signal bar at 08:15 GMT = 08:15 UTC
        let d = date(2024, 1, 15);
        let candles = make_day_candles(Instrument::Ftse, d, &[(7500.0, 7520.0, 7480.0, 7510.0)]);
        let config = StrategyConfig {
            instrument: Instrument::Ftse,
            ..default_config()
        };
        let result = find_signal_bar(&candles, Instrument::Ftse, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "08:15");
        assert_eq!(bar.buy_level, dec!(7522));
        assert_eq!(bar.sell_level, dec!(7478));
    }

    #[test]
    fn test_signal_bar_found_for_ftse_summer() {
        // 2024-07-15: BST (UTC+1), signal bar at 08:15 BST = 07:15 UTC
        let d = date(2024, 7, 15);
        let candles = make_day_candles(Instrument::Ftse, d, &[(7600.0, 7650.0, 7580.0, 7620.0)]);
        let config = StrategyConfig {
            instrument: Instrument::Ftse,
            ..default_config()
        };
        let result = find_signal_bar(&candles, Instrument::Ftse, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "07:15");
    }

    // -- Nasdaq tests --

    #[test]
    fn test_signal_bar_found_for_nasdaq_winter() {
        // 2024-01-15: EST (UTC-5), signal bar at 09:45 EST = 14:45 UTC
        let d = date(2024, 1, 15);
        let candles = make_day_candles(
            Instrument::Nasdaq,
            d,
            &[(15000.0, 15050.0, 14950.0, 15020.0)],
        );
        let config = StrategyConfig {
            instrument: Instrument::Nasdaq,
            ..default_config()
        };
        let result = find_signal_bar(&candles, Instrument::Nasdaq, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "14:45");
        assert_eq!(bar.buy_level, dec!(15052));
        assert_eq!(bar.sell_level, dec!(14948));
    }

    #[test]
    fn test_signal_bar_found_for_nasdaq_summer() {
        // 2024-07-15: EDT (UTC-4), signal bar at 09:45 EDT = 13:45 UTC
        let d = date(2024, 7, 15);
        let candles = make_day_candles(
            Instrument::Nasdaq,
            d,
            &[(16000.0, 16080.0, 15960.0, 16050.0)],
        );
        let config = StrategyConfig {
            instrument: Instrument::Nasdaq,
            ..default_config()
        };
        let result = find_signal_bar(&candles, Instrument::Nasdaq, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "13:45");
    }

    // -- Dow tests --

    #[test]
    fn test_signal_bar_found_for_dow_winter() {
        // 2024-01-15: EST (UTC-5), signal bar at 09:45 EST = 14:45 UTC
        let d = date(2024, 1, 15);
        let candles = make_day_candles(Instrument::Dow, d, &[(37000.0, 37100.0, 36950.0, 37050.0)]);
        let config = StrategyConfig {
            instrument: Instrument::Dow,
            ..default_config()
        };
        let result = find_signal_bar(&candles, Instrument::Dow, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "14:45");
        assert_eq!(bar.buy_level, dec!(37102));
        assert_eq!(bar.sell_level, dec!(36948));
    }

    // -- Missing data / edge cases --

    #[test]
    fn test_signal_bar_not_found_on_holiday() {
        let d = date(2024, 12, 25);
        let candles: Vec<Candle> = vec![];
        let config = default_config();
        let result = find_signal_bar(&candles, Instrument::Dax, d, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_signal_bar_not_found_when_candle_missing() {
        // Candles exist but not at the signal bar time
        let d = date(2024, 1, 15);
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16000",
            "16050",
            "15980",
            "16030",
        );
        let config = default_config();
        let result = find_signal_bar(&[candle], Instrument::Dax, d, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_signal_bar_custom_entry_offset() {
        let d = date(2024, 1, 15);
        let candles = make_day_candles(Instrument::Dax, d, &[(16000.0, 16050.0, 15980.0, 16030.0)]);
        let config = StrategyConfig {
            entry_offset_points: dec!(5),
            ..default_config()
        };
        let result = find_signal_bar(&candles, Instrument::Dax, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.buy_level, dec!(16055)); // 16050 + 5
        assert_eq!(bar.sell_level, dec!(15975)); // 15980 - 5
    }

    #[test]
    fn test_signal_bar_zero_entry_offset() {
        let d = date(2024, 1, 15);
        let candles = make_day_candles(Instrument::Dax, d, &[(16000.0, 16050.0, 15980.0, 16030.0)]);
        let config = StrategyConfig {
            entry_offset_points: dec!(0),
            ..default_config()
        };
        let result = find_signal_bar(&candles, Instrument::Dax, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.buy_level, dec!(16050)); // exactly at high
        assert_eq!(bar.sell_level, dec!(15980)); // exactly at low
    }

    #[test]
    fn test_signal_bar_serde_roundtrip() {
        let d = date(2024, 1, 15);
        let candles = make_day_candles(Instrument::Dax, d, &[(16000.0, 16050.0, 15980.0, 16030.0)]);
        let config = default_config();
        let bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();

        let json = serde_json::to_string(&bar).unwrap();
        let parsed: SignalBar = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.date, bar.date);
        assert_eq!(parsed.instrument, bar.instrument);
        assert_eq!(parsed.buy_level, bar.buy_level);
        assert_eq!(parsed.sell_level, bar.sell_level);
    }

    #[test]
    fn test_signal_bar_with_multiple_candles_finds_correct_one() {
        // Build candles that include pre-signal and post-signal bars
        let d = date(2024, 1, 15);
        let signal_utc = Instrument::Dax.signal_bar_start_utc(d).unwrap();

        // Pre-signal bar (15 min before signal bar time)
        let pre_bar = make_candle(
            Instrument::Dax,
            &(signal_utc - chrono::Duration::minutes(15))
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            "15900",
            "15950",
            "15880",
            "15920",
        );
        // Signal bar
        let signal_candle = make_candle(
            Instrument::Dax,
            &signal_utc.format("%Y-%m-%d %H:%M").to_string(),
            "16000",
            "16050",
            "15980",
            "16030",
        );
        // Post-signal bar (15 min after signal bar time)
        let post_bar = make_candle(
            Instrument::Dax,
            &(signal_utc + chrono::Duration::minutes(15))
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            "16030",
            "16080",
            "16010",
            "16060",
        );

        let candles = vec![pre_bar, signal_candle, post_bar];
        let config = default_config();
        let result = find_signal_bar(&candles, Instrument::Dax, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        // Should match the signal candle, not the pre or post bar
        assert_eq!(bar.candle.high, dec!(16050));
        assert_eq!(bar.candle.low, dec!(15980));
    }

    #[test]
    fn test_signal_bar_dst_transition_dax_spring() {
        // 2024-03-31: DST transition day. 09:15 CEST = 07:15 UTC.
        let d = date(2024, 3, 31);
        let candles = make_day_candles(Instrument::Dax, d, &[(17500.0, 17550.0, 17450.0, 17520.0)]);
        let config = default_config();
        let result = find_signal_bar(&candles, Instrument::Dax, d, &config);
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "07:15");
    }
}
