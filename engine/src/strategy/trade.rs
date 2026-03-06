//! Completed trade records.
//!
//! A [`Trade`] is produced when a [`Position`] is closed. It captures the
//! full lifecycle of the trade including entry, exit, PnL, and any
//! add-on positions.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::Instrument;

use super::config::StrategyConfig;
use super::position::{ExitResult, Position};
use super::types::{Direction, PositionStatus};

/// PnL result for a single add-on position within a trade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddResult {
    /// Price at which the add was filled.
    pub price: Decimal,
    /// Time of the add fill.
    pub time: DateTime<Utc>,
    /// Size of the add-on position.
    pub size: Decimal,
    /// Points gained or lost on this add-on position.
    pub pnl_points: Decimal,
}

/// A completed trade with full PnL computation.
///
/// Produced by [`Position::close`]. Includes the base position PnL
/// and all add-on position results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// The trading instrument.
    pub instrument: Instrument,
    /// Trade direction.
    pub direction: Direction,
    /// Entry fill price of the base position.
    pub entry_price: Decimal,
    /// Time of entry fill.
    pub entry_time: DateTime<Utc>,
    /// Exit fill price.
    pub exit_price: Decimal,
    /// Time of exit fill.
    pub exit_time: DateTime<Utc>,
    /// Stop loss level at exit (may have been tightened by adds).
    pub stop_loss: Decimal,
    /// Reason the position was closed.
    pub exit_reason: PositionStatus,
    /// Points gained or lost on the base position only.
    pub pnl_points: Decimal,
    /// Total PnL in points including all add-on positions.
    pub pnl_with_adds: Decimal,
    /// Results for each add-on position.
    pub adds: Vec<AddResult>,
    /// Base position size.
    pub size: Decimal,
}

impl Trade {
    /// Create a trade from a closed position and exit result.
    ///
    /// Computes PnL for the base position and all add-on positions.
    /// The direction multiplier ensures short trades produce positive
    /// PnL when price falls.
    #[must_use]
    pub(crate) fn from_position(
        position: Position,
        exit: ExitResult,
        config: &StrategyConfig,
    ) -> Self {
        let multiplier = position.direction_multiplier();
        let base_pnl = (exit.exit_price - position.entry_price) * multiplier;

        let mut add_results = Vec::with_capacity(position.adds.len());
        let mut total_add_pnl = Decimal::ZERO;

        for add in &position.adds {
            let add_pnl = (exit.exit_price - add.price) * multiplier;
            let weighted_pnl = add_pnl * add.size;
            total_add_pnl += weighted_pnl;
            add_results.push(AddResult {
                price: add.price,
                time: add.time,
                size: add.size,
                pnl_points: add_pnl,
            });
        }

        let pnl_with_adds = base_pnl * position.size + total_add_pnl;

        // Apply commission and slippage
        let total_fills = Decimal::ONE + Decimal::from(position.adds.len() as u64);
        let total_commission = config.commission_per_trade * total_fills;
        let total_slippage = config.slippage_points * total_fills * Decimal::new(2, 0);

        let pnl_with_adds = pnl_with_adds - total_commission - total_slippage;

        Self {
            instrument: config.instrument,
            direction: position.direction,
            entry_price: position.entry_price,
            entry_time: position.entry_time,
            exit_price: exit.exit_price,
            exit_time: exit.exit_time,
            stop_loss: position.stop_loss,
            exit_reason: exit.exit_reason,
            pnl_points: base_pnl,
            pnl_with_adds,
            adds: add_results,
            size: position.size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::position::AddPosition;
    use crate::test_helpers::utc;
    use rust_decimal_macros::dec;

    fn long_position() -> Position {
        Position {
            direction: Direction::Long,
            entry_price: dec!(16000),
            entry_time: utc(2024, 1, 15, 8, 30),
            stop_loss: dec!(15960),
            size: dec!(1),
            best_price: dec!(16050),
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
            best_price: dec!(15950),
            adds: Vec::new(),
            status: PositionStatus::Open,
        }
    }

    fn no_cost_config() -> StrategyConfig {
        StrategyConfig {
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        }
    }

    #[test]
    fn test_long_winning_trade_pnl() {
        let pos = long_position();
        let exit = ExitResult {
            exit_price: dec!(16050),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::TakeProfit,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        assert_eq!(trade.pnl_points, dec!(50));
        assert_eq!(trade.pnl_with_adds, dec!(50));
        assert_eq!(trade.direction, Direction::Long);
    }

    #[test]
    fn test_long_losing_trade_pnl() {
        let pos = long_position();
        let exit = ExitResult {
            exit_price: dec!(15960),
            exit_time: utc(2024, 1, 15, 9, 0),
            exit_reason: PositionStatus::StopLoss,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        assert_eq!(trade.pnl_points, dec!(-40));
        assert_eq!(trade.pnl_with_adds, dec!(-40));
    }

    #[test]
    fn test_short_winning_trade_pnl() {
        let pos = short_position();
        let exit = ExitResult {
            exit_price: dec!(15950),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::TakeProfit,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        assert_eq!(trade.pnl_points, dec!(50));
        assert_eq!(trade.pnl_with_adds, dec!(50));
        assert_eq!(trade.direction, Direction::Short);
    }

    #[test]
    fn test_short_losing_trade_pnl() {
        let pos = short_position();
        let exit = ExitResult {
            exit_price: dec!(16040),
            exit_time: utc(2024, 1, 15, 9, 0),
            exit_reason: PositionStatus::StopLoss,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        assert_eq!(trade.pnl_points, dec!(-40));
        assert_eq!(trade.pnl_with_adds, dec!(-40));
    }

    #[test]
    fn test_pnl_with_single_add() {
        let mut pos = long_position();
        pos.adds.push(AddPosition {
            price: dec!(16050),
            time: utc(2024, 1, 15, 9, 15),
            size: dec!(1),
            new_stop_loss: dec!(16000),
        });
        let exit = ExitResult {
            exit_price: dec!(16100),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::TakeProfit,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        // Base: (16100 - 16000) * 1 * 1 = 100
        // Add: (16100 - 16050) * (-1 for short? No, long) * 1 = 50 * 1 = 50
        // Total = 100 + 50 = 150
        assert_eq!(trade.pnl_points, dec!(100));
        assert_eq!(trade.pnl_with_adds, dec!(150));
        assert_eq!(trade.adds.len(), 1);
        assert_eq!(trade.adds[0].pnl_points, dec!(50));
    }

    #[test]
    fn test_pnl_with_multiple_adds() {
        let mut pos = long_position();
        pos.adds.push(AddPosition {
            price: dec!(16050),
            time: utc(2024, 1, 15, 9, 15),
            size: dec!(1),
            new_stop_loss: dec!(16000),
        });
        pos.adds.push(AddPosition {
            price: dec!(16100),
            time: utc(2024, 1, 15, 9, 30),
            size: dec!(1),
            new_stop_loss: dec!(16050),
        });
        let exit = ExitResult {
            exit_price: dec!(16150),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::EndOfDay,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        // Base: (16150 - 16000) * 1 = 150
        // Add1: (16150 - 16050) * 1 = 100
        // Add2: (16150 - 16100) * 1 = 50
        // Total = 150 + 100 + 50 = 300
        assert_eq!(trade.pnl_points, dec!(150));
        assert_eq!(trade.pnl_with_adds, dec!(300));
        assert_eq!(trade.adds.len(), 2);
    }

    #[test]
    fn test_pnl_with_adds_losing() {
        let mut pos = long_position();
        pos.adds.push(AddPosition {
            price: dec!(16050),
            time: utc(2024, 1, 15, 9, 15),
            size: dec!(1),
            new_stop_loss: dec!(16000),
        });
        pos.stop_loss = dec!(16000);
        let exit = ExitResult {
            exit_price: dec!(16000),
            exit_time: utc(2024, 1, 15, 9, 30),
            exit_reason: PositionStatus::StopLoss,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        // Base: (16000 - 16000) * 1 = 0
        // Add: (16000 - 16050) * 1 = -50
        // Total = 0 + (-50) = -50
        assert_eq!(trade.pnl_points, dec!(0));
        assert_eq!(trade.pnl_with_adds, dec!(-50));
    }

    #[test]
    fn test_commission_deducted() {
        let pos = long_position();
        let exit = ExitResult {
            exit_price: dec!(16050),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::TakeProfit,
        };
        let config = StrategyConfig {
            commission_per_trade: dec!(5),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };
        let trade = Trade::from_position(pos, exit, &config);
        // PnL = 50, commission = 5 * 1 fill = 5, slippage = 0
        assert_eq!(trade.pnl_points, dec!(50));
        assert_eq!(trade.pnl_with_adds, dec!(45));
    }

    #[test]
    fn test_slippage_deducted() {
        let pos = long_position();
        let exit = ExitResult {
            exit_price: dec!(16050),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::TakeProfit,
        };
        let config = StrategyConfig {
            commission_per_trade: dec!(0),
            slippage_points: dec!(0.5),
            ..StrategyConfig::default()
        };
        let trade = Trade::from_position(pos, exit, &config);
        // PnL = 50, slippage = 0.5 * 1 * 2 = 1.0
        assert_eq!(trade.pnl_points, dec!(50));
        assert_eq!(trade.pnl_with_adds, dec!(49));
    }

    #[test]
    fn test_commission_and_slippage_with_adds() {
        let mut pos = long_position();
        pos.adds.push(AddPosition {
            price: dec!(16050),
            time: utc(2024, 1, 15, 9, 15),
            size: dec!(1),
            new_stop_loss: dec!(16000),
        });
        let exit = ExitResult {
            exit_price: dec!(16100),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::TakeProfit,
        };
        let config = StrategyConfig {
            commission_per_trade: dec!(5),
            slippage_points: dec!(0.5),
            ..StrategyConfig::default()
        };
        let trade = Trade::from_position(pos, exit, &config);
        // Base PnL: 100, Add PnL: 50 -> raw = 150
        // 2 fills: commission = 5 * 2 = 10, slippage = 0.5 * 2 * 2 = 2
        // Net = 150 - 10 - 2 = 138
        assert_eq!(trade.pnl_points, dec!(100));
        assert_eq!(trade.pnl_with_adds, dec!(138));
    }

    #[test]
    fn test_trade_exit_reason_preserved() {
        let pos = long_position();
        let exit = ExitResult {
            exit_price: dec!(16055),
            exit_time: utc(2024, 1, 15, 16, 30),
            exit_reason: PositionStatus::EndOfDay,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        assert_eq!(trade.exit_reason, PositionStatus::EndOfDay);
    }

    #[test]
    fn test_trade_serde_roundtrip() {
        let pos = long_position();
        let exit = ExitResult {
            exit_price: dec!(16050),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::TakeProfit,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        let json = serde_json::to_string(&trade).unwrap();
        let parsed: Trade = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pnl_points, trade.pnl_points);
        assert_eq!(parsed.entry_price, trade.entry_price);
        assert_eq!(parsed.exit_price, trade.exit_price);
        assert_eq!(parsed.direction, trade.direction);
    }

    #[test]
    fn test_short_trade_with_adds_pnl() {
        let mut pos = short_position();
        pos.adds.push(AddPosition {
            price: dec!(15950),
            time: utc(2024, 1, 15, 9, 15),
            size: dec!(1),
            new_stop_loss: dec!(16000),
        });
        let exit = ExitResult {
            exit_price: dec!(15900),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::TakeProfit,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        // Base: (15900 - 16000) * -1 = 100
        // Add: (15900 - 15950) * -1 * 1 = 50
        // Total = 100 + 50 = 150
        assert_eq!(trade.pnl_points, dec!(100));
        assert_eq!(trade.pnl_with_adds, dec!(150));
    }

    #[test]
    fn test_trade_with_different_add_sizes() {
        let mut pos = long_position();
        pos.adds.push(AddPosition {
            price: dec!(16050),
            time: utc(2024, 1, 15, 9, 15),
            size: dec!(2),
            new_stop_loss: dec!(16000),
        });
        let exit = ExitResult {
            exit_price: dec!(16100),
            exit_time: utc(2024, 1, 15, 10, 0),
            exit_reason: PositionStatus::TakeProfit,
        };
        let trade = Trade::from_position(pos, exit, &no_cost_config());
        // Base: (16100 - 16000) * 1 = 100 (size 1)
        // Add: (16100 - 16050) * 1 = 50, weighted by size 2 = 100
        // Total = 100 + 100 = 200
        assert_eq!(trade.pnl_points, dec!(100));
        assert_eq!(trade.pnl_with_adds, dec!(200));
    }
}
