//! Order generation and stop loss computation.
//!
//! Given a [`SignalBar`], this module generates [`PendingOrder`]s with the
//! correct trigger prices and stop loss levels based on the strategy
//! configuration.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::config::StrategyConfig;
use super::signal::SignalBar;
use super::types::{Direction, StopLossMode};

/// A pending entry order waiting to be filled.
///
/// The order activates when price reaches `trigger_price`:
/// - Long: buy stop triggers when price trades at or above `trigger_price`
/// - Short: sell stop triggers when price trades at or below `trigger_price`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOrder {
    /// Long (buy stop) or Short (sell stop).
    pub direction: Direction,
    /// Price at which the stop order triggers.
    pub trigger_price: Decimal,
    /// Initial stop loss price.
    pub stop_loss: Decimal,
    /// Position size in lots/contracts.
    pub size: Decimal,
    /// Time after which this order expires. `None` means valid for the session.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Generates pending entry orders from a detected signal bar.
///
/// By default, two orders are generated:
/// - A **buy stop** at `signal_bar.buy_level` (high + offset)
/// - A **sell stop** at `signal_bar.sell_level` (low - offset)
///
/// Stop loss is computed based on the configured [`StopLossMode`]:
/// - `SignalBarExtreme`: Long SL = signal bar low, Short SL = signal bar high
/// - `FixedPoints`: Long SL = trigger - fixed points, Short SL = trigger + fixed points
/// - `Midpoint`: Long SL = midpoint - offset, Short SL = midpoint + offset
///
/// When `sl_scale_with_index` is enabled, the fixed stop loss distance is
/// scaled proportionally: `sl_fixed_points * (current_price / sl_scale_baseline)`.
///
/// # Arguments
///
/// * `signal_bar` - The detected signal bar with pre-computed buy/sell levels
/// * `config` - Strategy configuration
#[must_use]
pub fn generate_orders(signal_bar: &SignalBar, config: &StrategyConfig) -> Vec<PendingOrder> {
    let candle = &signal_bar.candle;
    let midpoint = (candle.high + candle.low) / Decimal::new(2, 0);

    let buy_trigger = signal_bar.buy_level;
    let sell_trigger = signal_bar.sell_level;

    let sl_fixed = if config.sl_scale_with_index {
        let current_price = candle.close;
        scale_stop_loss(
            config.sl_fixed_points,
            current_price,
            config.sl_scale_baseline,
        )
    } else {
        config.sl_fixed_points
    };

    let buy_sl = compute_stop_loss_long(
        config.sl_mode,
        buy_trigger,
        candle.low,
        midpoint,
        sl_fixed,
        config.sl_midpoint_offset,
    );
    let sell_sl = compute_stop_loss_short(
        config.sl_mode,
        sell_trigger,
        candle.high,
        midpoint,
        sl_fixed,
        config.sl_midpoint_offset,
    );

    let mut orders = Vec::with_capacity(2);

    orders.push(PendingOrder {
        direction: Direction::Long,
        trigger_price: buy_trigger,
        stop_loss: buy_sl,
        size: config.position_size,
        expires_at: None,
    });

    if config.allow_both_sides {
        orders.push(PendingOrder {
            direction: Direction::Short,
            trigger_price: sell_trigger,
            stop_loss: sell_sl,
            size: config.position_size,
            expires_at: None,
        });
    }

    orders
}

/// Computes the stop loss for a long position.
fn compute_stop_loss_long(
    mode: StopLossMode,
    trigger_price: Decimal,
    signal_bar_low: Decimal,
    midpoint: Decimal,
    sl_fixed: Decimal,
    sl_midpoint_offset: Decimal,
) -> Decimal {
    match mode {
        StopLossMode::SignalBarExtreme => signal_bar_low,
        StopLossMode::FixedPoints => trigger_price - sl_fixed,
        StopLossMode::Midpoint => midpoint - sl_midpoint_offset,
    }
}

/// Computes the stop loss for a short position.
fn compute_stop_loss_short(
    mode: StopLossMode,
    trigger_price: Decimal,
    signal_bar_high: Decimal,
    midpoint: Decimal,
    sl_fixed: Decimal,
    sl_midpoint_offset: Decimal,
) -> Decimal {
    match mode {
        StopLossMode::SignalBarExtreme => signal_bar_high,
        StopLossMode::FixedPoints => trigger_price + sl_fixed,
        StopLossMode::Midpoint => midpoint + sl_midpoint_offset,
    }
}

/// Scales the fixed stop loss distance proportionally to the current index level.
///
/// `scaled = sl_fixed_points * (current_price / baseline)`
fn scale_stop_loss(sl_fixed_points: Decimal, current_price: Decimal, baseline: Decimal) -> Decimal {
    if baseline.is_zero() {
        return sl_fixed_points;
    }
    sl_fixed_points * current_price / baseline
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Instrument;
    use crate::strategy::config::StrategyConfig;
    use crate::test_helpers::{date, make_day_candles};
    use rust_decimal_macros::dec;

    fn make_signal_bar(instrument: Instrument, high: f64, low: f64, close: f64) -> SignalBar {
        let d = date(2024, 1, 15);
        let candles = make_day_candles(instrument, d, &[(100.0, high, low, close)]);
        SignalBar {
            date: d,
            instrument,
            candle: candles.into_iter().next().unwrap(),
            buy_level: Decimal::try_from(high).unwrap() + dec!(2),
            sell_level: Decimal::try_from(low).unwrap() - dec!(2),
        }
    }

    // -- SignalBarExtreme SL mode --

    #[test]
    fn test_generate_orders_signal_bar_extreme_dax() {
        let bar = make_signal_bar(Instrument::Dax, 16050.0, 15980.0, 16030.0);
        let config = StrategyConfig {
            sl_mode: StopLossMode::SignalBarExtreme,
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders.len(), 2);

        let buy = &orders[0];
        assert_eq!(buy.direction, Direction::Long);
        assert_eq!(buy.trigger_price, dec!(16052));
        assert_eq!(buy.stop_loss, dec!(15980)); // signal bar low

        let sell = &orders[1];
        assert_eq!(sell.direction, Direction::Short);
        assert_eq!(sell.trigger_price, dec!(15978));
        assert_eq!(sell.stop_loss, dec!(16050)); // signal bar high
    }

    #[test]
    fn test_generate_orders_signal_bar_extreme_ftse() {
        let bar = make_signal_bar(Instrument::Ftse, 7520.0, 7480.0, 7510.0);
        let config = StrategyConfig {
            instrument: Instrument::Ftse,
            sl_mode: StopLossMode::SignalBarExtreme,
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].stop_loss, dec!(7480));
        assert_eq!(orders[1].stop_loss, dec!(7520));
    }

    #[test]
    fn test_generate_orders_signal_bar_extreme_nasdaq() {
        let bar = make_signal_bar(Instrument::Nasdaq, 15050.0, 14950.0, 15020.0);
        let config = StrategyConfig {
            instrument: Instrument::Nasdaq,
            sl_mode: StopLossMode::SignalBarExtreme,
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders[0].stop_loss, dec!(14950));
        assert_eq!(orders[1].stop_loss, dec!(15050));
    }

    #[test]
    fn test_generate_orders_signal_bar_extreme_dow() {
        let bar = make_signal_bar(Instrument::Dow, 37100.0, 36950.0, 37050.0);
        let config = StrategyConfig {
            instrument: Instrument::Dow,
            sl_mode: StopLossMode::SignalBarExtreme,
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders[0].stop_loss, dec!(36950));
        assert_eq!(orders[1].stop_loss, dec!(37100));
    }

    // -- FixedPoints SL mode --

    #[test]
    fn test_generate_orders_fixed_points_dax() {
        let bar = make_signal_bar(Instrument::Dax, 16050.0, 15980.0, 16030.0);
        let config = StrategyConfig {
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);

        let buy = &orders[0];
        assert_eq!(buy.stop_loss, dec!(16012)); // 16052 - 40

        let sell = &orders[1];
        assert_eq!(sell.stop_loss, dec!(16018)); // 15978 + 40
    }

    #[test]
    fn test_generate_orders_fixed_points_ftse() {
        let bar = make_signal_bar(Instrument::Ftse, 7520.0, 7480.0, 7510.0);
        let config = StrategyConfig {
            instrument: Instrument::Ftse,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(30),
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders[0].stop_loss, dec!(7492)); // 7522 - 30
        assert_eq!(orders[1].stop_loss, dec!(7508)); // 7478 + 30
    }

    #[test]
    fn test_generate_orders_fixed_points_nasdaq() {
        let bar = make_signal_bar(Instrument::Nasdaq, 15050.0, 14950.0, 15020.0);
        let config = StrategyConfig {
            instrument: Instrument::Nasdaq,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders[0].stop_loss, dec!(15012)); // 15052 - 40
        assert_eq!(orders[1].stop_loss, dec!(14988)); // 14948 + 40
    }

    #[test]
    fn test_generate_orders_fixed_points_dow() {
        let bar = make_signal_bar(Instrument::Dow, 37100.0, 36950.0, 37050.0);
        let config = StrategyConfig {
            instrument: Instrument::Dow,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(50),
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders[0].stop_loss, dec!(37052)); // 37102 - 50
        assert_eq!(orders[1].stop_loss, dec!(36998)); // 36948 + 50
    }

    // -- Midpoint SL mode --

    #[test]
    fn test_generate_orders_midpoint_dax() {
        let bar = make_signal_bar(Instrument::Dax, 16050.0, 15980.0, 16030.0);
        // midpoint = (16050 + 15980) / 2 = 16015
        let config = StrategyConfig {
            sl_mode: StopLossMode::Midpoint,
            sl_midpoint_offset: dec!(5),
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders[0].stop_loss, dec!(16010)); // 16015 - 5
        assert_eq!(orders[1].stop_loss, dec!(16020)); // 16015 + 5
    }

    #[test]
    fn test_generate_orders_midpoint_ftse() {
        let bar = make_signal_bar(Instrument::Ftse, 7520.0, 7480.0, 7510.0);
        // midpoint = (7520 + 7480) / 2 = 7500
        let config = StrategyConfig {
            instrument: Instrument::Ftse,
            sl_mode: StopLossMode::Midpoint,
            sl_midpoint_offset: dec!(3),
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders[0].stop_loss, dec!(7497)); // 7500 - 3
        assert_eq!(orders[1].stop_loss, dec!(7503)); // 7500 + 3
    }

    #[test]
    fn test_generate_orders_midpoint_nasdaq() {
        let bar = make_signal_bar(Instrument::Nasdaq, 15050.0, 14950.0, 15020.0);
        // midpoint = (15050 + 14950) / 2 = 15000
        let config = StrategyConfig {
            instrument: Instrument::Nasdaq,
            sl_mode: StopLossMode::Midpoint,
            sl_midpoint_offset: dec!(10),
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders[0].stop_loss, dec!(14990)); // 15000 - 10
        assert_eq!(orders[1].stop_loss, dec!(15010)); // 15000 + 10
    }

    #[test]
    fn test_generate_orders_midpoint_dow() {
        let bar = make_signal_bar(Instrument::Dow, 37100.0, 36900.0, 37000.0);
        // midpoint = (37100 + 36900) / 2 = 37000
        let config = StrategyConfig {
            instrument: Instrument::Dow,
            sl_mode: StopLossMode::Midpoint,
            sl_midpoint_offset: dec!(5),
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders[0].stop_loss, dec!(36995)); // 37000 - 5
        assert_eq!(orders[1].stop_loss, dec!(37005)); // 37000 + 5
    }

    // -- Edge cases --

    #[test]
    fn test_generate_orders_flat_candle() {
        // high == low (flat candle)
        let bar = make_signal_bar(Instrument::Dax, 16000.0, 16000.0, 16000.0);
        let config = StrategyConfig {
            sl_mode: StopLossMode::SignalBarExtreme,
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders.len(), 2);
        // Both SLs are at the same level (signal bar extreme is 16000 for both)
        assert_eq!(orders[0].stop_loss, dec!(16000));
        assert_eq!(orders[1].stop_loss, dec!(16000));
        // Buy trigger above, sell trigger below
        assert_eq!(orders[0].trigger_price, dec!(16002));
        assert_eq!(orders[1].trigger_price, dec!(15998));
    }

    #[test]
    fn test_generate_orders_allow_both_sides_false() {
        let bar = make_signal_bar(Instrument::Dax, 16050.0, 15980.0, 16030.0);
        let config = StrategyConfig {
            allow_both_sides: false,
            sl_mode: StopLossMode::FixedPoints,
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].direction, Direction::Long);
    }

    #[test]
    fn test_generate_orders_position_size() {
        let bar = make_signal_bar(Instrument::Dax, 16050.0, 15980.0, 16030.0);
        let config = StrategyConfig {
            position_size: dec!(2.5),
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        assert_eq!(orders[0].size, dec!(2.5));
        assert_eq!(orders[1].size, dec!(2.5));
    }

    #[test]
    fn test_generate_orders_expires_at_is_none() {
        let bar = make_signal_bar(Instrument::Dax, 16050.0, 15980.0, 16030.0);
        let config = StrategyConfig::default();
        let orders = generate_orders(&bar, &config);
        assert!(orders[0].expires_at.is_none());
        assert!(orders[1].expires_at.is_none());
    }

    // -- Scaling --

    #[test]
    fn test_generate_orders_scaled_stop_loss() {
        let bar = make_signal_bar(Instrument::Dax, 18100.0, 18000.0, 18050.0);
        let config = StrategyConfig {
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            sl_scale_with_index: true,
            sl_scale_baseline: dec!(12000),
            ..StrategyConfig::default()
        };
        let orders = generate_orders(&bar, &config);
        // scaled SL = 40 * 18050 / 12000 = 60.1666...
        // buy trigger = 18102, buy SL = 18102 - 60.1666... = 18041.833...
        let buy = &orders[0];
        assert_eq!(buy.direction, Direction::Long);
        // Verify SL is less than trigger but more than the unscaled version
        assert!(buy.stop_loss < buy.trigger_price);
        assert!(buy.stop_loss > buy.trigger_price - dec!(70));
    }

    #[test]
    fn test_scale_stop_loss_same_as_baseline() {
        let result = scale_stop_loss(dec!(40), dec!(12000), dec!(12000));
        assert_eq!(result, dec!(40));
    }

    #[test]
    fn test_scale_stop_loss_double_baseline() {
        let result = scale_stop_loss(dec!(40), dec!(24000), dec!(12000));
        assert_eq!(result, dec!(80));
    }

    #[test]
    fn test_scale_stop_loss_zero_baseline() {
        let result = scale_stop_loss(dec!(40), dec!(18000), dec!(0));
        assert_eq!(result, dec!(40)); // fallback to unscaled
    }

    #[test]
    fn test_order_serde_roundtrip() {
        let bar = make_signal_bar(Instrument::Dax, 16050.0, 15980.0, 16030.0);
        let config = StrategyConfig::default();
        let orders = generate_orders(&bar, &config);

        for order in &orders {
            let json = serde_json::to_string(order).unwrap();
            let parsed: PendingOrder = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.direction, order.direction);
            assert_eq!(parsed.trigger_price, order.trigger_price);
            assert_eq!(parsed.stop_loss, order.stop_loss);
            assert_eq!(parsed.size, order.size);
        }
    }
}
