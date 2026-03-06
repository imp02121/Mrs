//! Adding to winners logic.
//!
//! When a trade moves favorably, the strategy can add additional positions
//! at configured intervals. Each add can optionally tighten the stop loss.
//! See [`check_add_trigger`] for the core logic.

use rust_decimal::Decimal;

use crate::models::Candle;

use super::config::StrategyConfig;
use super::position::{AddPosition, Position};
use super::types::Direction;

/// Check whether the current candle triggers an add-to-winners entry.
///
/// An add triggers when price moves `add_every_points * (num_adds + 1)`
/// from the original entry in the favorable direction. For example, with
/// `add_every_points = 50` and no prior adds, the first add triggers at
/// +50 points. The second at +100 points, etc.
///
/// Returns `None` if:
/// - Adding is disabled in config
/// - Maximum additions have been reached
/// - Price hasn't moved far enough
///
/// When `move_sl_on_add` is true, the new stop loss is set to the previous
/// add's entry price (or the original entry for the first add), offset by
/// `add_sl_offset`.
#[must_use]
pub fn check_add_trigger(
    position: &Position,
    candle: &Candle,
    config: &StrategyConfig,
) -> Option<AddPosition> {
    if !config.add_to_winners_enabled {
        return None;
    }

    let num_adds = position.adds.len();
    if num_adds >= config.max_additions as usize {
        return None;
    }

    let next_add_number = num_adds + 1;
    let required_move = config.add_every_points * Decimal::from(next_add_number as u64);

    let (trigger_price, hit) = match position.direction {
        Direction::Long => {
            let trigger = position.entry_price + required_move;
            (trigger, candle.high >= trigger)
        }
        Direction::Short => {
            let trigger = position.entry_price - required_move;
            (trigger, candle.low <= trigger)
        }
    };

    if !hit {
        return None;
    }

    let new_stop_loss = if config.move_sl_on_add {
        compute_tightened_sl(position, config)
    } else {
        position.stop_loss
    };

    let add_size = config.position_size * config.add_size_multiplier;

    Some(AddPosition {
        price: trigger_price,
        time: candle.timestamp,
        size: add_size,
        new_stop_loss,
    })
}

/// Compute the tightened stop loss when adding to a winner.
///
/// The new SL is set to the most recent add's entry price (or the original
/// entry if no prior adds), offset by `add_sl_offset` in the adverse direction.
fn compute_tightened_sl(position: &Position, config: &StrategyConfig) -> Decimal {
    let reference_price = position
        .adds
        .last()
        .map(|a| a.price)
        .unwrap_or(position.entry_price);

    match position.direction {
        Direction::Long => reference_price - config.add_sl_offset,
        Direction::Short => reference_price + config.add_sl_offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Instrument;
    use crate::strategy::types::PositionStatus;
    use crate::test_helpers::{make_candle, utc};
    use rust_decimal_macros::dec;

    fn base_position() -> Position {
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

    fn add_config() -> StrategyConfig {
        StrategyConfig {
            add_to_winners_enabled: true,
            add_every_points: dec!(50),
            max_additions: 3,
            add_size_multiplier: dec!(1),
            move_sl_on_add: true,
            add_sl_offset: dec!(0),
            position_size: dec!(1),
            ..StrategyConfig::default()
        }
    }

    #[test]
    fn test_first_add_triggers_at_correct_level_long() {
        let pos = base_position();
        let config = add_config();
        // First add at entry + 50 = 16050. Candle high = 16060 >= 16050
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16040",
            "16060",
            "16030",
            "16055",
        );
        let result = check_add_trigger(&pos, &candle, &config);
        assert!(result.is_some());
        let add = result.unwrap();
        assert_eq!(add.price, dec!(16050));
        assert_eq!(add.size, dec!(1));
    }

    #[test]
    fn test_first_add_triggers_at_correct_level_short() {
        let mut pos = base_position();
        pos.direction = Direction::Short;
        pos.stop_loss = dec!(16040);
        let config = add_config();
        // First add at entry - 50 = 15950. Candle low = 15940 <= 15950
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "15960",
            "15970",
            "15940",
            "15945",
        );
        let result = check_add_trigger(&pos, &candle, &config);
        assert!(result.is_some());
        let add = result.unwrap();
        assert_eq!(add.price, dec!(15950));
    }

    #[test]
    fn test_second_add_requires_double_distance() {
        let mut pos = base_position();
        // Already has one add
        pos.adds.push(AddPosition {
            price: dec!(16050),
            time: utc(2024, 1, 15, 9, 0),
            size: dec!(1),
            new_stop_loss: dec!(16000),
        });
        let config = add_config();
        // Second add at entry + 100 = 16100. Candle high = 16110
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:30",
            "16090",
            "16110",
            "16080",
            "16105",
        );
        let result = check_add_trigger(&pos, &candle, &config);
        assert!(result.is_some());
        let add = result.unwrap();
        assert_eq!(add.price, dec!(16100));
    }

    #[test]
    fn test_max_adds_enforced() {
        let mut pos = base_position();
        // Already has 3 adds (max_additions = 3)
        for i in 1..=3 {
            pos.adds.push(AddPosition {
                price: dec!(16000) + Decimal::from(i) * dec!(50),
                time: utc(2024, 1, 15, 9, i as u32 * 15),
                size: dec!(1),
                new_stop_loss: dec!(16000) + Decimal::from(i - 1) * dec!(50),
            });
        }
        let config = add_config();
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 10:30",
            "16190",
            "16210",
            "16180",
            "16205",
        );
        let result = check_add_trigger(&pos, &candle, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_sl_tightening_on_first_add() {
        let pos = base_position();
        let config = StrategyConfig {
            add_sl_offset: dec!(5),
            ..add_config()
        };
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16040",
            "16060",
            "16030",
            "16055",
        );
        let result = check_add_trigger(&pos, &candle, &config);
        assert!(result.is_some());
        let add = result.unwrap();
        // First add: reference = entry(16000), new SL = 16000 - 5 = 15995
        assert_eq!(add.new_stop_loss, dec!(15995));
    }

    #[test]
    fn test_sl_tightening_on_second_add() {
        let mut pos = base_position();
        pos.adds.push(AddPosition {
            price: dec!(16050),
            time: utc(2024, 1, 15, 9, 0),
            size: dec!(1),
            new_stop_loss: dec!(15995),
        });
        let config = StrategyConfig {
            add_sl_offset: dec!(5),
            ..add_config()
        };
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:30",
            "16090",
            "16110",
            "16080",
            "16105",
        );
        let result = check_add_trigger(&pos, &candle, &config);
        assert!(result.is_some());
        let add = result.unwrap();
        // Second add: reference = last add price(16050), new SL = 16050 - 5 = 16045
        assert_eq!(add.new_stop_loss, dec!(16045));
    }

    #[test]
    fn test_no_trigger_when_disabled() {
        let pos = base_position();
        let config = StrategyConfig {
            add_to_winners_enabled: false,
            ..add_config()
        };
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16040",
            "16060",
            "16030",
            "16055",
        );
        let result = check_add_trigger(&pos, &candle, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_no_trigger_when_price_not_far_enough() {
        let pos = base_position();
        let config = add_config();
        // First add needs 16050, candle high only 16040
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16020",
            "16040",
            "16010",
            "16035",
        );
        let result = check_add_trigger(&pos, &candle, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_no_sl_tightening_when_disabled() {
        let pos = base_position();
        let config = StrategyConfig {
            move_sl_on_add: false,
            ..add_config()
        };
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16040",
            "16060",
            "16030",
            "16055",
        );
        let result = check_add_trigger(&pos, &candle, &config);
        assert!(result.is_some());
        let add = result.unwrap();
        // SL should remain unchanged
        assert_eq!(add.new_stop_loss, dec!(15960));
    }

    #[test]
    fn test_add_size_multiplier() {
        let pos = base_position();
        let config = StrategyConfig {
            add_size_multiplier: dec!(2),
            position_size: dec!(1),
            ..add_config()
        };
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16040",
            "16060",
            "16030",
            "16055",
        );
        let result = check_add_trigger(&pos, &candle, &config);
        assert!(result.is_some());
        assert_eq!(result.unwrap().size, dec!(2));
    }
}
