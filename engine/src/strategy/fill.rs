//! Fill simulation for pending stop orders.
//!
//! Determines whether a pending buy/sell stop order would be filled on a given
//! candle, and at what price. Handles gap opens, slippage, and the case where
//! both buy and sell stops trigger on the same candle.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::Candle;

use super::config::StrategyConfig;
use super::order::PendingOrder;
use super::types::Direction;

/// The result of a fill: direction, price, time, and the originating order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillResult {
    /// The direction of the filled order.
    pub direction: Direction,
    /// The actual fill price (after gap adjustment and slippage).
    pub fill_price: Decimal,
    /// The timestamp of the candle on which the fill occurred.
    pub fill_time: DateTime<Utc>,
    /// The original pending order that was filled.
    pub order: PendingOrder,
}

/// Checks whether a pending order would be filled on the given candle.
///
/// **Buy stop**: fills when `candle.high >= trigger_price`.
/// Fill price is `max(trigger_price, candle.open)` to handle gap-up opens.
///
/// **Sell stop**: fills when `candle.low <= trigger_price`.
/// Fill price is `min(trigger_price, candle.open)` to handle gap-down opens.
///
/// Slippage is applied to the fill price: added for longs, subtracted for shorts.
///
/// Returns `None` if the order does not trigger on this candle.
///
/// # Arguments
///
/// * `order` - The pending stop order to check
/// * `candle` - The candle to check against
/// * `slippage` - Slippage in points to apply
#[must_use]
pub fn check_fill(order: &PendingOrder, candle: &Candle, slippage: Decimal) -> Option<FillResult> {
    match order.direction {
        Direction::Long => {
            if candle.high >= order.trigger_price {
                // Gap-up: fill at open if open is above trigger
                let raw_price = order.trigger_price.max(candle.open);
                let fill_price = raw_price + slippage;
                Some(FillResult {
                    direction: Direction::Long,
                    fill_price,
                    fill_time: candle.timestamp,
                    order: order.clone(),
                })
            } else {
                None
            }
        }
        Direction::Short => {
            if candle.low <= order.trigger_price {
                // Gap-down: fill at open if open is below trigger
                let raw_price = order.trigger_price.min(candle.open);
                let fill_price = raw_price - slippage;
                Some(FillResult {
                    direction: Direction::Short,
                    fill_price,
                    fill_time: candle.timestamp,
                    order: order.clone(),
                })
            } else {
                None
            }
        }
    }
}

/// When both buy and sell stops could fill on the same candle, determines
/// which fills first based on proximity to the candle's open.
///
/// The order closest to `candle.open` triggers first. If both are equidistant,
/// the buy order fills first (arbitrary but deterministic).
///
/// Slippage from the config is applied to each fill.
///
/// # Arguments
///
/// * `buy` - The pending buy stop order
/// * `sell` - The pending sell stop order
/// * `candle` - The candle to check against
/// * `config` - Strategy config (for slippage)
#[must_use]
pub fn determine_fill_order(
    buy: &PendingOrder,
    sell: &PendingOrder,
    candle: &Candle,
    config: &StrategyConfig,
) -> Vec<FillResult> {
    let slippage = config.slippage_points;
    let buy_fill = check_fill(buy, candle, slippage);
    let sell_fill = check_fill(sell, candle, slippage);

    match (buy_fill, sell_fill) {
        (Some(b), Some(s)) => {
            let buy_distance = (candle.open - buy.trigger_price).abs();
            let sell_distance = (candle.open - sell.trigger_price).abs();
            if buy_distance <= sell_distance {
                vec![b, s]
            } else {
                vec![s, b]
            }
        }
        (Some(b), None) => vec![b],
        (None, Some(s)) => vec![s],
        (None, None) => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Instrument;
    use crate::test_helpers::make_candle;
    use rust_decimal_macros::dec;

    fn buy_order(trigger: Decimal, sl: Decimal) -> PendingOrder {
        PendingOrder {
            direction: Direction::Long,
            trigger_price: trigger,
            stop_loss: sl,
            size: dec!(1),
            expires_at: None,
        }
    }

    fn sell_order(trigger: Decimal, sl: Decimal) -> PendingOrder {
        PendingOrder {
            direction: Direction::Short,
            trigger_price: trigger,
            stop_loss: sl,
            size: dec!(1),
            expires_at: None,
        }
    }

    // -- Normal fill tests --

    #[test]
    fn test_buy_stop_fills_when_high_reaches_trigger() {
        let order = buy_order(dec!(16052), dec!(16012));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16040",
            "16060",
            "16035",
            "16055",
        );
        let result = check_fill(&order, &candle, dec!(0));
        assert!(result.is_some());
        let fill = result.unwrap();
        assert_eq!(fill.direction, Direction::Long);
        assert_eq!(fill.fill_price, dec!(16052)); // trigger price (open < trigger)
    }

    #[test]
    fn test_sell_stop_fills_when_low_reaches_trigger() {
        let order = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "15990",
            "16000",
            "15970",
            "15985",
        );
        let result = check_fill(&order, &candle, dec!(0));
        assert!(result.is_some());
        let fill = result.unwrap();
        assert_eq!(fill.direction, Direction::Short);
        assert_eq!(fill.fill_price, dec!(15978)); // trigger price (open > trigger)
    }

    // -- Gap fill tests --

    #[test]
    fn test_buy_stop_gap_up_fills_at_open() {
        // Candle opens above the trigger price (gap up)
        let order = buy_order(dec!(16052), dec!(16012));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16070",
            "16080",
            "16065",
            "16075",
        );
        let result = check_fill(&order, &candle, dec!(0));
        assert!(result.is_some());
        let fill = result.unwrap();
        assert_eq!(fill.fill_price, dec!(16070)); // fills at open, not trigger
    }

    #[test]
    fn test_sell_stop_gap_down_fills_at_open() {
        // Candle opens below the trigger price (gap down)
        let order = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "15960",
            "15970",
            "15950",
            "15965",
        );
        let result = check_fill(&order, &candle, dec!(0));
        assert!(result.is_some());
        let fill = result.unwrap();
        assert_eq!(fill.fill_price, dec!(15960)); // fills at open, not trigger
    }

    // -- No fill tests --

    #[test]
    fn test_buy_stop_no_fill_when_high_below_trigger() {
        let order = buy_order(dec!(16052), dec!(16012));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16020",
            "16040",
            "16010",
            "16030",
        );
        let result = check_fill(&order, &candle, dec!(0));
        assert!(result.is_none());
    }

    #[test]
    fn test_sell_stop_no_fill_when_low_above_trigger() {
        let order = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16000",
            "16020",
            "15990",
            "16010",
        );
        let result = check_fill(&order, &candle, dec!(0));
        assert!(result.is_none());
    }

    // -- Slippage tests --

    #[test]
    fn test_buy_stop_with_slippage() {
        let order = buy_order(dec!(16052), dec!(16012));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16040",
            "16060",
            "16035",
            "16055",
        );
        let result = check_fill(&order, &candle, dec!(0.5));
        assert!(result.is_some());
        assert_eq!(result.unwrap().fill_price, dec!(16052.5)); // trigger + slippage
    }

    #[test]
    fn test_sell_stop_with_slippage() {
        let order = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "15990",
            "16000",
            "15970",
            "15985",
        );
        let result = check_fill(&order, &candle, dec!(0.5));
        assert!(result.is_some());
        assert_eq!(result.unwrap().fill_price, dec!(15977.5)); // trigger - slippage
    }

    // -- Both sides triggered --

    #[test]
    fn test_both_sides_triggered_buy_closer_to_open() {
        // Open is 16050, buy trigger at 16052 (distance 2), sell trigger at 15978 (distance 72)
        let buy = buy_order(dec!(16052), dec!(16012));
        let sell = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16050",
            "16060",
            "15970",
            "15990",
        );
        let config = StrategyConfig {
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };
        let fills = determine_fill_order(&buy, &sell, &candle, &config);
        assert_eq!(fills.len(), 2);
        assert_eq!(fills[0].direction, Direction::Long); // buy is closer to open
        assert_eq!(fills[1].direction, Direction::Short);
    }

    #[test]
    fn test_both_sides_triggered_sell_closer_to_open() {
        // Open is 15980, buy trigger at 16052 (distance 72), sell trigger at 15978 (distance 2)
        let buy = buy_order(dec!(16052), dec!(16012));
        let sell = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "15980",
            "16060",
            "15970",
            "16000",
        );
        let config = StrategyConfig {
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };
        let fills = determine_fill_order(&buy, &sell, &candle, &config);
        assert_eq!(fills.len(), 2);
        assert_eq!(fills[0].direction, Direction::Short); // sell is closer to open
        assert_eq!(fills[1].direction, Direction::Long);
    }

    #[test]
    fn test_both_sides_equidistant_buy_first() {
        // Open is 16015, buy trigger at 16052 (distance 37), sell trigger at 15978 (distance 37)
        let buy = buy_order(dec!(16052), dec!(16012));
        let sell = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16015",
            "16060",
            "15970",
            "16000",
        );
        let config = StrategyConfig {
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };
        let fills = determine_fill_order(&buy, &sell, &candle, &config);
        assert_eq!(fills.len(), 2);
        assert_eq!(fills[0].direction, Direction::Long); // buy wins tie
    }

    #[test]
    fn test_only_buy_triggered() {
        let buy = buy_order(dec!(16052), dec!(16012));
        let sell = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16040",
            "16060",
            "16000",
            "16055",
        );
        let config = StrategyConfig {
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };
        let fills = determine_fill_order(&buy, &sell, &candle, &config);
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].direction, Direction::Long);
    }

    #[test]
    fn test_only_sell_triggered() {
        let buy = buy_order(dec!(16052), dec!(16012));
        let sell = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "15990",
            "16000",
            "15970",
            "15985",
        );
        let config = StrategyConfig {
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };
        let fills = determine_fill_order(&buy, &sell, &candle, &config);
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].direction, Direction::Short);
    }

    #[test]
    fn test_neither_triggered() {
        let buy = buy_order(dec!(16052), dec!(16012));
        let sell = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16010",
            "16040",
            "15990",
            "16020",
        );
        let config = StrategyConfig {
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };
        let fills = determine_fill_order(&buy, &sell, &candle, &config);
        assert!(fills.is_empty());
    }

    // -- Fill result tests --

    #[test]
    fn test_fill_result_contains_original_order() {
        let order = buy_order(dec!(16052), dec!(16012));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16040",
            "16060",
            "16035",
            "16055",
        );
        let fill = check_fill(&order, &candle, dec!(0)).unwrap();
        assert_eq!(fill.order.trigger_price, dec!(16052));
        assert_eq!(fill.order.stop_loss, dec!(16012));
    }

    #[test]
    fn test_fill_result_timestamp_matches_candle() {
        let order = buy_order(dec!(16052), dec!(16012));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16040",
            "16060",
            "16035",
            "16055",
        );
        let fill = check_fill(&order, &candle, dec!(0)).unwrap();
        assert_eq!(fill.fill_time, candle.timestamp);
    }

    #[test]
    fn test_fill_result_serde_roundtrip() {
        let order = buy_order(dec!(16052), dec!(16012));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16040",
            "16060",
            "16035",
            "16055",
        );
        let fill = check_fill(&order, &candle, dec!(0)).unwrap();

        let json = serde_json::to_string(&fill).unwrap();
        let parsed: FillResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.direction, fill.direction);
        assert_eq!(parsed.fill_price, fill.fill_price);
        assert_eq!(parsed.fill_time, fill.fill_time);
    }

    #[test]
    fn test_buy_stop_exact_touch_fills() {
        // High exactly equals trigger price
        let order = buy_order(dec!(16052), dec!(16012));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "16040",
            "16052",
            "16035",
            "16045",
        );
        let result = check_fill(&order, &candle, dec!(0));
        assert!(result.is_some());
        assert_eq!(result.unwrap().fill_price, dec!(16052));
    }

    #[test]
    fn test_sell_stop_exact_touch_fills() {
        // Low exactly equals trigger price
        let order = sell_order(dec!(15978), dec!(16018));
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 08:30",
            "15990",
            "16000",
            "15978",
            "15985",
        );
        let result = check_fill(&order, &candle, dec!(0));
        assert!(result.is_some());
        assert_eq!(result.unwrap().fill_price, dec!(15978));
    }
}
