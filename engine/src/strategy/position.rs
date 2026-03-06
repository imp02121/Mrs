//! Position tracking and per-candle update logic.
//!
//! A [`Position`] represents an open trade with a direction, entry price,
//! stop loss, and optional add-on positions. The [`Position::update`] method
//! processes a single candle against the position, checking exits in the
//! correct priority order (stop loss first, then trailing, take profit,
//! adding, and time-based exits).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::Candle;

use super::add_to_winners::check_add_trigger;
use super::config::StrategyConfig;
use super::trade::Trade;
use super::types::{Direction, ExitMode, PositionStatus};

/// An additional position added to a winning trade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddPosition {
    /// Price at which the add was filled.
    pub price: Decimal,
    /// Time of the add fill.
    pub time: DateTime<Utc>,
    /// Size of the additional position.
    pub size: Decimal,
    /// The new stop loss level after this add (if SL was tightened).
    pub new_stop_loss: Decimal,
}

/// The result of a position being closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitResult {
    /// Price at which the position was closed.
    pub exit_price: Decimal,
    /// Time of the exit.
    pub exit_time: DateTime<Utc>,
    /// Reason the position was closed.
    pub exit_reason: PositionStatus,
}

/// An open trading position.
///
/// Tracks direction, entry, stop loss, best favorable price (for trailing
/// stops), and any add-on positions. Use [`Position::update`] to process
/// each subsequent candle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// Trade direction (long or short).
    pub direction: Direction,
    /// Entry fill price.
    pub entry_price: Decimal,
    /// Time of entry fill.
    pub entry_time: DateTime<Utc>,
    /// Current stop loss level.
    pub stop_loss: Decimal,
    /// Position size in lots/contracts.
    pub size: Decimal,
    /// Best favorable price seen since entry (high for longs, low for shorts).
    pub best_price: Decimal,
    /// Additional positions added to this trade.
    pub adds: Vec<AddPosition>,
    /// Current status of the position.
    pub status: PositionStatus,
}

impl Position {
    /// Returns `1` for long positions, `-1` for short positions.
    ///
    /// Used to normalize PnL calculations: `(exit - entry) * multiplier`.
    #[must_use]
    pub fn direction_multiplier(&self) -> Decimal {
        match self.direction {
            Direction::Long => Decimal::ONE,
            Direction::Short => Decimal::NEGATIVE_ONE,
        }
    }

    /// Process a single candle against this position.
    ///
    /// Checks exit conditions in the following order (critical for correctness):
    /// 1. Stop loss hit (conservative: assumed first when ambiguous)
    /// 2. Update best price
    /// 3. Trailing stop
    /// 4. Take profit
    /// 5. Adding to winners
    /// 6. Time-based exit (end-of-day or close-at-time)
    ///
    /// Returns `Some(ExitResult)` if the position was closed, `None` if still open.
    pub fn update(&mut self, candle: &Candle, config: &StrategyConfig) -> Option<ExitResult> {
        // 1. Check stop loss hit first (conservative assumption)
        if let Some(exit) = self.check_stop_loss(candle) {
            self.status = PositionStatus::StopLoss;
            return Some(exit);
        }

        // 2. Update best_price
        self.update_best_price(candle);

        // 3. Check trailing stop
        if config.exit_mode == ExitMode::TrailingStop
            && let Some(exit) = self.check_trailing_stop(candle, config)
        {
            self.status = PositionStatus::TrailingStop;
            return Some(exit);
        }

        // 4. Check take profit
        if config.exit_mode == ExitMode::FixedTakeProfit
            && let Some(exit) = self.check_take_profit(candle, config)
        {
            self.status = PositionStatus::TakeProfit;
            return Some(exit);
        }

        // 5. Check adding conditions
        if config.add_to_winners_enabled
            && let Some(add) = check_add_trigger(self, candle, config)
        {
            self.stop_loss = add.new_stop_loss;
            self.adds.push(add);
        }

        // 6. Check time-based exit
        if let Some(exit) = self.check_time_exit(candle, config) {
            return Some(exit);
        }

        None
    }

    /// Check if the stop loss was hit on this candle.
    ///
    /// For longs: SL hit if `candle.low <= stop_loss`.
    /// For shorts: SL hit if `candle.high >= stop_loss`.
    fn check_stop_loss(&self, candle: &Candle) -> Option<ExitResult> {
        let hit = match self.direction {
            Direction::Long => candle.low <= self.stop_loss,
            Direction::Short => candle.high >= self.stop_loss,
        };

        if hit {
            Some(ExitResult {
                exit_price: self.stop_loss,
                exit_time: candle.timestamp,
                exit_reason: PositionStatus::StopLoss,
            })
        } else {
            None
        }
    }

    /// Update the best favorable price seen.
    ///
    /// For longs, tracks the highest high; for shorts, tracks the lowest low.
    fn update_best_price(&mut self, candle: &Candle) {
        match self.direction {
            Direction::Long => {
                if candle.high > self.best_price {
                    self.best_price = candle.high;
                }
            }
            Direction::Short => {
                if candle.low < self.best_price {
                    self.best_price = candle.low;
                }
            }
        }
    }

    /// Check if the trailing stop has been hit.
    ///
    /// The trailing stop activates only after unrealized profit exceeds
    /// `trailing_stop_activation`. Once active, it trails at
    /// `trailing_stop_distance` from `best_price`.
    fn check_trailing_stop(&self, candle: &Candle, config: &StrategyConfig) -> Option<ExitResult> {
        let unrealized = (self.best_price - self.entry_price) * self.direction_multiplier();
        if unrealized < config.trailing_stop_activation {
            return None;
        }

        let trail_level = match self.direction {
            Direction::Long => self.best_price - config.trailing_stop_distance,
            Direction::Short => self.best_price + config.trailing_stop_distance,
        };

        let hit = match self.direction {
            Direction::Long => candle.low <= trail_level,
            Direction::Short => candle.high >= trail_level,
        };

        if hit {
            Some(ExitResult {
                exit_price: trail_level,
                exit_time: candle.timestamp,
                exit_reason: PositionStatus::TrailingStop,
            })
        } else {
            None
        }
    }

    /// Check if the take profit level has been reached.
    fn check_take_profit(&self, candle: &Candle, config: &StrategyConfig) -> Option<ExitResult> {
        let tp_level = match self.direction {
            Direction::Long => self.entry_price + config.fixed_tp_points,
            Direction::Short => self.entry_price - config.fixed_tp_points,
        };

        let hit = match self.direction {
            Direction::Long => candle.high >= tp_level,
            Direction::Short => candle.low <= tp_level,
        };

        if hit {
            Some(ExitResult {
                exit_price: tp_level,
                exit_time: candle.timestamp,
                exit_reason: PositionStatus::TakeProfit,
            })
        } else {
            None
        }
    }

    /// Check time-based exit conditions (end-of-day or close-at-time).
    ///
    /// Converts the candle timestamp to exchange local time and compares
    /// against the configured exit time.
    fn check_time_exit(&mut self, candle: &Candle, config: &StrategyConfig) -> Option<ExitResult> {
        let exit_time_local = match config.exit_mode {
            ExitMode::EndOfDay => config.exit_eod_time,
            ExitMode::CloseAtTime => config.close_at_time,
            _ => return None,
        };

        let tz = config.instrument.exchange_timezone();
        let candle_local = candle.timestamp.with_timezone(&tz);
        let candle_time = candle_local.time();

        if candle_time >= exit_time_local {
            let reason = match config.exit_mode {
                ExitMode::EndOfDay => PositionStatus::EndOfDay,
                ExitMode::CloseAtTime => PositionStatus::TimeClose,
                _ => unreachable!(),
            };
            self.status = reason;
            Some(ExitResult {
                exit_price: candle.close,
                exit_time: candle.timestamp,
                exit_reason: reason,
            })
        } else {
            None
        }
    }

    /// Close this position and produce a completed [`Trade`] record.
    ///
    /// Consumes the position, computing PnL for the base position and
    /// all add-on positions.
    #[must_use]
    pub fn close(mut self, exit: ExitResult, config: &StrategyConfig) -> Trade {
        self.status = exit.exit_reason;
        Trade::from_position(self, exit, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Instrument;
    use crate::test_helpers::{make_candle, utc};
    use rust_decimal_macros::dec;

    fn long_position() -> Position {
        Position {
            direction: Direction::Long,
            entry_price: dec!(16000),
            entry_time: utc(2024, 1, 15, 8, 30),
            stop_loss: dec!(15960),
            size: dec!(1),
            best_price: dec!(16000),
            adds: Vec::new(),
            status: PositionStatus::Open,
        }
    }

    fn short_position() -> Position {
        Position {
            direction: Direction::Short,
            entry_price: dec!(16000),
            entry_time: utc(2024, 1, 15, 8, 30),
            stop_loss: dec!(16040),
            size: dec!(1),
            best_price: dec!(16000),
            adds: Vec::new(),
            status: PositionStatus::Open,
        }
    }

    fn default_config() -> StrategyConfig {
        StrategyConfig::default()
    }

    #[test]
    fn test_direction_multiplier_long() {
        let pos = long_position();
        assert_eq!(pos.direction_multiplier(), dec!(1));
    }

    #[test]
    fn test_direction_multiplier_short() {
        let pos = short_position();
        assert_eq!(pos.direction_multiplier(), dec!(-1));
    }

    #[test]
    fn test_stop_loss_hit_long() {
        let mut pos = long_position();
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:45",
            "15980",
            "15990",
            "15950",
            "15970",
        );
        let config = default_config();
        let result = pos.update(&candle, &config);
        assert!(result.is_some());
        let exit = result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::StopLoss);
        assert_eq!(exit.exit_price, dec!(15960));
    }

    #[test]
    fn test_stop_loss_hit_short() {
        let mut pos = short_position();
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:45",
            "16020",
            "16050",
            "16010",
            "16045",
        );
        let config = default_config();
        let result = pos.update(&candle, &config);
        assert!(result.is_some());
        let exit = result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::StopLoss);
        assert_eq!(exit.exit_price, dec!(16040));
    }

    #[test]
    fn test_no_exit_when_price_within_range() {
        let mut pos = long_position();
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:45",
            "16010",
            "16020",
            "15970",
            "16015",
        );
        let config = default_config();
        let result = pos.update(&candle, &config);
        assert!(result.is_none());
        assert_eq!(pos.best_price, dec!(16020));
    }

    #[test]
    fn test_best_price_updates_long() {
        let mut pos = long_position();
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:45",
            "16010",
            "16050",
            "15970",
            "16040",
        );
        let config = default_config();
        let _ = pos.update(&candle, &config);
        assert_eq!(pos.best_price, dec!(16050));
    }

    #[test]
    fn test_best_price_updates_short() {
        let mut pos = short_position();
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:45",
            "15990",
            "16030",
            "15970",
            "15980",
        );
        let config = default_config();
        let _ = pos.update(&candle, &config);
        assert_eq!(pos.best_price, dec!(15970));
    }

    #[test]
    fn test_trailing_stop_long() {
        let mut pos = long_position();
        pos.best_price = dec!(16050);
        let config = StrategyConfig {
            exit_mode: ExitMode::TrailingStop,
            trailing_stop_distance: dec!(30),
            trailing_stop_activation: dec!(0),
            ..default_config()
        };
        // Trail level = 16050 - 30 = 16020. Candle low = 16015 <= 16020 -> hit
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16030",
            "16040",
            "16015",
            "16020",
        );
        let result = pos.update(&candle, &config);
        assert!(result.is_some());
        let exit = result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TrailingStop);
        assert_eq!(exit.exit_price, dec!(16020));
    }

    #[test]
    fn test_trailing_stop_short() {
        let mut pos = short_position();
        pos.best_price = dec!(15950);
        let config = StrategyConfig {
            exit_mode: ExitMode::TrailingStop,
            trailing_stop_distance: dec!(30),
            trailing_stop_activation: dec!(0),
            ..default_config()
        };
        // Trail level = 15950 + 30 = 15980. Candle high = 15985 >= 15980 -> hit
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "15960",
            "15985",
            "15955",
            "15975",
        );
        let result = pos.update(&candle, &config);
        assert!(result.is_some());
        let exit = result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TrailingStop);
        assert_eq!(exit.exit_price, dec!(15980));
    }

    #[test]
    fn test_trailing_stop_not_activated_below_threshold() {
        let mut pos = long_position();
        pos.best_price = dec!(16010);
        let config = StrategyConfig {
            exit_mode: ExitMode::TrailingStop,
            trailing_stop_distance: dec!(30),
            trailing_stop_activation: dec!(20),
            ..default_config()
        };
        // Unrealized = 16010 - 16000 = 10 < activation(20), so trailing not active
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16005",
            "16010",
            "15975",
            "15980",
        );
        let result = pos.update(&candle, &config);
        // Should not trigger trailing stop, but SL at 15960 is not hit (low=15975)
        assert!(result.is_none());
    }

    #[test]
    fn test_take_profit_long() {
        let mut pos = long_position();
        let config = StrategyConfig {
            exit_mode: ExitMode::FixedTakeProfit,
            fixed_tp_points: dec!(100),
            ..default_config()
        };
        // TP level = 16000 + 100 = 16100. Candle high = 16110 >= 16100 -> hit
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 10:00",
            "16080",
            "16110",
            "16070",
            "16105",
        );
        let result = pos.update(&candle, &config);
        assert!(result.is_some());
        let exit = result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TakeProfit);
        assert_eq!(exit.exit_price, dec!(16100));
    }

    #[test]
    fn test_take_profit_short() {
        let mut pos = short_position();
        let config = StrategyConfig {
            exit_mode: ExitMode::FixedTakeProfit,
            fixed_tp_points: dec!(100),
            ..default_config()
        };
        // TP level = 16000 - 100 = 15900. Candle low = 15890 <= 15900 -> hit
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 10:00",
            "15920",
            "15930",
            "15890",
            "15895",
        );
        let result = pos.update(&candle, &config);
        assert!(result.is_some());
        let exit = result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TakeProfit);
        assert_eq!(exit.exit_price, dec!(15900));
    }

    #[test]
    fn test_end_of_day_exit() {
        let mut pos = long_position();
        let config = default_config(); // exit_mode = EndOfDay, exit_eod_time = 17:30
        // 2024-01-15 is winter CET (UTC+1). 17:30 CET = 16:30 UTC.
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 16:30",
            "16050",
            "16060",
            "16040",
            "16055",
        );
        let result = pos.update(&candle, &config);
        assert!(result.is_some());
        let exit = result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::EndOfDay);
        assert_eq!(exit.exit_price, dec!(16055)); // closes at candle.close
    }

    #[test]
    fn test_close_at_time_exit() {
        let mut pos = long_position();
        let config = StrategyConfig {
            exit_mode: ExitMode::CloseAtTime,
            close_at_time: chrono::NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
            ..default_config()
        };
        // 15:00 CET = 14:00 UTC in winter
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 14:00",
            "16020",
            "16030",
            "16010",
            "16025",
        );
        let result = pos.update(&candle, &config);
        assert!(result.is_some());
        let exit = result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TimeClose);
        assert_eq!(exit.exit_price, dec!(16025));
    }

    #[test]
    fn test_sl_takes_priority_over_favorable_price() {
        // When both SL and TP could be hit in same candle, SL wins (conservative)
        let mut pos = long_position(); // entry 16000, SL 15960
        let config = StrategyConfig {
            exit_mode: ExitMode::FixedTakeProfit,
            fixed_tp_points: dec!(50),
            ..default_config()
        };
        // TP = 16050. Candle has high >= 16050 AND low <= 15960 -> SL first
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16000",
            "16060",
            "15950",
            "16020",
        );
        let result = pos.update(&candle, &config);
        assert!(result.is_some());
        let exit = result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::StopLoss);
        assert_eq!(exit.exit_price, dec!(15960));
    }

    #[test]
    fn test_position_close_produces_trade() {
        let pos = long_position();
        let exit = ExitResult {
            exit_price: dec!(16050),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::TakeProfit,
        };
        let config = default_config();
        let trade = pos.close(exit, &config);
        assert_eq!(trade.direction, Direction::Long);
        assert_eq!(trade.entry_price, dec!(16000));
        assert_eq!(trade.exit_price, dec!(16050));
        assert_eq!(trade.pnl_points, dec!(50));
    }

    #[test]
    fn test_no_exit_before_eod_time() {
        let mut pos = long_position();
        let config = default_config(); // EndOfDay at 17:30 CET
        // 12:00 CET = 11:00 UTC in winter -> before 17:30
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 11:00",
            "16010",
            "16020",
            "15970",
            "16015",
        );
        let result = pos.update(&candle, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_stop_loss_exact_level_triggers() {
        let mut pos = long_position(); // SL at 15960
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:45",
            "15980",
            "15990",
            "15960",
            "15970",
        );
        let config = default_config();
        let result = pos.update(&candle, &config);
        assert!(result.is_some());
        assert_eq!(result.unwrap().exit_reason, PositionStatus::StopLoss);
    }
}
