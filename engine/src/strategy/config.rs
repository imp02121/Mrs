//! Strategy configuration.
//!
//! [`StrategyConfig`] holds all parameters needed to run the School Run
//! Strategy: signal detection, stop loss, exit, adding to winners,
//! session timing, and backtest scope. All financial values use [`Decimal`].

use chrono::{NaiveDate, NaiveTime};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::Instrument;

use super::types::{ExitMode, StopLossMode};

/// Complete configuration for a School Run Strategy backtest or signal run.
///
/// Use [`StrategyConfig::default()`] for sensible defaults matching the
/// original Tom Hougaard parameters (DAX, fixed 40-point stop, end-of-day
/// exit, no adding to winners).
///
/// All point-based values use [`Decimal`] for exact financial arithmetic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    // -- Signal Detection --
    /// The trading instrument (determines session times and timezone).
    pub instrument: Instrument,

    /// Which 15-minute candle after market open to use as the signal bar.
    ///
    /// Index is 1-based from the first candle after open:
    /// - `1` = first candle (e.g. 09:00-09:15 for DAX)
    /// - `2` = second candle (e.g. 09:15-09:30 for DAX) -- Hougaard's default
    pub signal_bar_index: u8,

    /// Candle interval in minutes. Default 15.
    ///
    /// Allows experimentation with other timeframes (5, 10, 30).
    pub candle_interval_minutes: u16,

    /// Points above signal bar high (buy stop) or below low (sell stop).
    pub entry_offset_points: Decimal,

    /// Whether both buy and sell stop orders can trigger in the same session.
    pub allow_both_sides: bool,

    // -- Stop Loss --
    /// How the initial stop loss is determined.
    pub sl_mode: StopLossMode,

    /// Fixed stop loss distance in points (used when `sl_mode = FixedPoints`).
    pub sl_fixed_points: Decimal,

    /// Offset beyond the midpoint for midpoint stop loss mode.
    pub sl_midpoint_offset: Decimal,

    /// Whether to scale the stop loss proportionally to the current index level.
    pub sl_scale_with_index: bool,

    /// The index level at which `sl_fixed_points` was calibrated.
    pub sl_scale_baseline: Decimal,

    // -- Exit Strategy --
    /// How and when positions are exited.
    pub exit_mode: ExitMode,

    /// Time to flatten positions in end-of-day mode (exchange local time).
    pub exit_eod_time: NaiveTime,

    /// Trailing stop distance in points.
    pub trailing_stop_distance: Decimal,

    /// Minimum unrealized profit before the trailing stop activates.
    pub trailing_stop_activation: Decimal,

    /// Fixed take profit distance in points.
    pub fixed_tp_points: Decimal,

    /// Time to close all positions in close-at-time mode (exchange local time).
    pub close_at_time: NaiveTime,

    // -- Adding to Winners --
    /// Whether to add to winning positions.
    pub add_to_winners_enabled: bool,

    /// Add an additional position every X points of favorable movement.
    pub add_every_points: Decimal,

    /// Maximum number of additional entries per trade.
    pub max_additions: u8,

    /// Size of each add relative to the initial position (`1.0` = same size).
    pub add_size_multiplier: Decimal,

    /// Whether to tighten the stop loss when adding to winners.
    pub move_sl_on_add: bool,

    /// Offset from the previous add's entry price for the new stop loss.
    pub add_sl_offset: Decimal,

    // -- Session Timing Overrides --
    /// Override for session open time (exchange local time).
    /// When `None`, uses the instrument's default.
    pub session_open: Option<NaiveTime>,

    /// Override for session close time (exchange local time).
    /// When `None`, uses the instrument's default.
    pub session_close: Option<NaiveTime>,

    /// Time after which unfilled entry orders are cancelled.
    /// `None` means orders remain active for the entire session.
    pub signal_expiry_time: Option<NaiveTime>,

    // -- Backtest Scope --
    /// Start date for the backtest (inclusive).
    pub date_from: NaiveDate,

    /// End date for the backtest (inclusive).
    pub date_to: NaiveDate,

    /// Starting capital for the equity curve.
    pub initial_capital: Decimal,

    /// Base position size in lots/contracts.
    pub position_size: Decimal,

    /// Cash value per point per lot.
    pub point_value: Decimal,

    /// Round-trip commission cost per trade.
    pub commission_per_trade: Decimal,

    /// Simulated slippage per fill in points.
    pub slippage_points: Decimal,

    /// Dates to exclude from backtesting (holidays, data gaps).
    pub exclude_dates: Vec<NaiveDate>,
}

impl Default for StrategyConfig {
    /// Returns a default configuration matching the original School Run
    /// Strategy parameters: DAX, 2nd candle, 2-point offset, fixed 40-point
    /// stop, end-of-day exit, no adding to winners.
    fn default() -> Self {
        Self {
            // Signal detection
            instrument: Instrument::Dax,
            signal_bar_index: 2,
            candle_interval_minutes: 15,
            entry_offset_points: Decimal::new(2, 0),
            allow_both_sides: true,

            // Stop loss
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: Decimal::new(40, 0),
            sl_midpoint_offset: Decimal::new(5, 0),
            sl_scale_with_index: false,
            sl_scale_baseline: Decimal::new(12000, 0),

            // Exit strategy
            exit_mode: ExitMode::EndOfDay,
            exit_eod_time: NaiveTime::from_hms_opt(17, 30, 0).expect("hardcoded valid time"),
            trailing_stop_distance: Decimal::new(30, 0),
            trailing_stop_activation: Decimal::ZERO,
            fixed_tp_points: Decimal::new(100, 0),
            close_at_time: NaiveTime::from_hms_opt(15, 0, 0).expect("hardcoded valid time"),

            // Adding to winners
            add_to_winners_enabled: false,
            add_every_points: Decimal::new(50, 0),
            max_additions: 3,
            add_size_multiplier: Decimal::ONE,
            move_sl_on_add: true,
            add_sl_offset: Decimal::ZERO,

            // Session timing overrides
            session_open: None,
            session_close: None,
            signal_expiry_time: None,

            // Backtest scope
            date_from: NaiveDate::from_ymd_opt(2024, 1, 1).expect("hardcoded valid date"),
            date_to: NaiveDate::from_ymd_opt(2025, 12, 31).expect("hardcoded valid date"),
            initial_capital: Decimal::new(100_000, 0),
            position_size: Decimal::ONE,
            point_value: Decimal::ONE,
            commission_per_trade: Decimal::ZERO,
            slippage_points: Decimal::new(5, 1), // 0.5
            exclude_dates: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_default_config_signal_detection() {
        let config = StrategyConfig::default();
        assert_eq!(config.instrument, Instrument::Dax);
        assert_eq!(config.signal_bar_index, 2);
        assert_eq!(config.candle_interval_minutes, 15);
        assert_eq!(config.entry_offset_points, dec!(2));
        assert!(config.allow_both_sides);
    }

    #[test]
    fn test_default_config_stop_loss() {
        let config = StrategyConfig::default();
        assert_eq!(config.sl_mode, StopLossMode::FixedPoints);
        assert_eq!(config.sl_fixed_points, dec!(40));
        assert_eq!(config.sl_midpoint_offset, dec!(5));
        assert!(!config.sl_scale_with_index);
        assert_eq!(config.sl_scale_baseline, dec!(12000));
    }

    #[test]
    fn test_default_config_exit() {
        let config = StrategyConfig::default();
        assert_eq!(config.exit_mode, ExitMode::EndOfDay);
        assert_eq!(
            config.exit_eod_time,
            NaiveTime::from_hms_opt(17, 30, 0).unwrap()
        );
        assert_eq!(config.trailing_stop_distance, dec!(30));
        assert_eq!(config.trailing_stop_activation, dec!(0));
        assert_eq!(config.fixed_tp_points, dec!(100));
        assert_eq!(
            config.close_at_time,
            NaiveTime::from_hms_opt(15, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_default_config_adding_to_winners() {
        let config = StrategyConfig::default();
        assert!(!config.add_to_winners_enabled);
        assert_eq!(config.add_every_points, dec!(50));
        assert_eq!(config.max_additions, 3);
        assert_eq!(config.add_size_multiplier, dec!(1));
        assert!(config.move_sl_on_add);
        assert_eq!(config.add_sl_offset, dec!(0));
    }

    #[test]
    fn test_default_config_session_timing() {
        let config = StrategyConfig::default();
        assert!(config.session_open.is_none());
        assert!(config.session_close.is_none());
        assert!(config.signal_expiry_time.is_none());
    }

    #[test]
    fn test_default_config_backtest_scope() {
        let config = StrategyConfig::default();
        assert_eq!(
            config.date_from,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()
        );
        assert_eq!(
            config.date_to,
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()
        );
        assert_eq!(config.initial_capital, dec!(100000));
        assert_eq!(config.position_size, dec!(1));
        assert_eq!(config.point_value, dec!(1));
        assert_eq!(config.commission_per_trade, dec!(0));
        assert_eq!(config.slippage_points, dec!(0.5));
        assert!(config.exclude_dates.is_empty());
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = StrategyConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: StrategyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.instrument, config.instrument);
        assert_eq!(parsed.signal_bar_index, config.signal_bar_index);
        assert_eq!(parsed.entry_offset_points, config.entry_offset_points);
        assert_eq!(parsed.sl_mode, config.sl_mode);
        assert_eq!(parsed.exit_mode, config.exit_mode);
        assert_eq!(parsed.date_from, config.date_from);
        assert_eq!(parsed.initial_capital, config.initial_capital);
    }

    #[test]
    fn test_config_clone() {
        let config = StrategyConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.instrument, config.instrument);
        assert_eq!(cloned.sl_fixed_points, config.sl_fixed_points);
    }

    #[test]
    fn test_config_custom_values() {
        let config = StrategyConfig {
            instrument: Instrument::Ftse,
            signal_bar_index: 3,
            entry_offset_points: dec!(5),
            sl_mode: StopLossMode::SignalBarExtreme,
            exit_mode: ExitMode::TrailingStop,
            trailing_stop_distance: dec!(25),
            add_to_winners_enabled: true,
            max_additions: 5,
            ..StrategyConfig::default()
        };
        assert_eq!(config.instrument, Instrument::Ftse);
        assert_eq!(config.signal_bar_index, 3);
        assert_eq!(config.entry_offset_points, dec!(5));
        assert_eq!(config.sl_mode, StopLossMode::SignalBarExtreme);
        assert_eq!(config.exit_mode, ExitMode::TrailingStop);
        assert_eq!(config.trailing_stop_distance, dec!(25));
        assert!(config.add_to_winners_enabled);
        assert_eq!(config.max_additions, 5);
        // Ensure defaults are preserved for unset fields
        assert_eq!(config.sl_fixed_points, dec!(40));
        assert_eq!(config.candle_interval_minutes, 15);
    }

    #[test]
    fn test_config_serde_with_optional_fields() {
        let config = StrategyConfig {
            session_open: Some(NaiveTime::from_hms_opt(10, 0, 0).unwrap()),
            signal_expiry_time: Some(NaiveTime::from_hms_opt(12, 0, 0).unwrap()),
            ..StrategyConfig::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: StrategyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.session_open,
            Some(NaiveTime::from_hms_opt(10, 0, 0).unwrap())
        );
        assert_eq!(
            parsed.signal_expiry_time,
            Some(NaiveTime::from_hms_opt(12, 0, 0).unwrap())
        );
        assert!(parsed.session_close.is_none());
    }

    #[test]
    fn test_config_with_exclude_dates() {
        let config = StrategyConfig {
            exclude_dates: vec![
                NaiveDate::from_ymd_opt(2024, 12, 25).unwrap(),
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            ],
            ..StrategyConfig::default()
        };
        assert_eq!(config.exclude_dates.len(), 2);

        let json = serde_json::to_string(&config).unwrap();
        let parsed: StrategyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.exclude_dates.len(), 2);
    }
}
