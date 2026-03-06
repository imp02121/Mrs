//! Full-flow integration tests for the School Run Strategy.
//!
//! These tests exercise the complete pipeline: candles -> signal bar ->
//! orders -> fill -> position updates -> trade close. They verify that
//! all modules integrate correctly and produce expected known-answer results.

#[cfg(test)]
mod tests {
    use crate::models::Instrument;
    use crate::strategy::config::StrategyConfig;
    use crate::strategy::fill::{check_fill, determine_fill_order};
    use crate::strategy::order::generate_orders;
    use crate::strategy::position::{ExitResult, Position};
    use crate::strategy::signal::find_signal_bar;
    use crate::strategy::types::{Direction, ExitMode, PositionStatus, StopLossMode};
    use crate::test_helpers::{date, make_day_candles};
    use rust_decimal_macros::dec;

    // =========================================================================
    // Full-flow integration tests
    // =========================================================================

    /// Complete flow: DAX winter day, long trade triggers, runs into EOD close.
    #[test]
    fn test_full_flow_dax_long_eod_close() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::EndOfDay,
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        // Build candles for the day:
        // Bar 0 (08:15 UTC = signal bar): O=16000 H=16050 L=15980 C=16030
        // Bar 1 (08:30 UTC): Price breaks above buy level (16052), triggering long
        // Bar 2-6: Price moves up gradually
        // Bar 33 (16:30 UTC = 17:30 CET winter): EOD close candle
        let mut bars: Vec<(f64, f64, f64, f64)> = Vec::new();
        // Signal bar
        bars.push((16000.0, 16050.0, 15980.0, 16030.0));
        // Bar 1: breaks above 16052 (buy level)
        bars.push((16040.0, 16060.0, 16035.0, 16055.0));
        // Bars 2-32: gradual uptrend, no SL hit (SL is at 16012)
        for i in 2..33 {
            let base = 16055.0 + (i as f64) * 2.0;
            bars.push((base, base + 10.0, base - 5.0, base + 5.0));
        }
        // Bar 33 (16:30 UTC = 17:30 CET): EOD close
        bars.push((16120.0, 16130.0, 16110.0, 16125.0));

        let candles = make_day_candles(Instrument::Dax, d, &bars);

        // Step 1: Find signal bar
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        assert_eq!(signal_bar.buy_level, dec!(16052));
        assert_eq!(signal_bar.sell_level, dec!(15978));

        // Step 2: Generate orders
        let orders = generate_orders(&signal_bar, &config);
        assert_eq!(orders.len(), 2);
        let buy_order = &orders[0];
        assert_eq!(buy_order.direction, Direction::Long);
        assert_eq!(buy_order.trigger_price, dec!(16052));
        assert_eq!(buy_order.stop_loss, dec!(16012)); // 16052 - 40

        // Step 3: Check fill on bar 1 (08:30 UTC)
        let fill = check_fill(buy_order, &candles[1], dec!(0)).unwrap();
        assert_eq!(fill.fill_price, dec!(16052));
        assert_eq!(fill.direction, Direction::Long);

        // Step 4: Create position from fill
        let mut position = Position {
            direction: fill.direction,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: buy_order.stop_loss,
            size: config.position_size,
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        // Step 5: Update position with subsequent candles
        let mut exit_result: Option<ExitResult> = None;
        for candle in &candles[2..] {
            if let Some(exit) = position.update(candle, &config) {
                exit_result = Some(exit);
                break;
            }
        }

        // Step 6: Should exit at EOD
        let exit = exit_result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::EndOfDay);
        assert_eq!(exit.exit_price, dec!(16125)); // close of EOD candle

        // Step 7: Close position to get trade
        let trade = position.close(exit, &config);
        assert_eq!(trade.direction, Direction::Long);
        assert_eq!(trade.entry_price, dec!(16052));
        assert_eq!(trade.exit_price, dec!(16125));
        assert_eq!(trade.pnl_points, dec!(73)); // 16125 - 16052
        assert!(trade.adds.is_empty());
    }

    /// Complete flow: DAX short trade triggers, hits stop loss.
    #[test]
    fn test_full_flow_dax_short_stop_loss() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::EndOfDay,
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let bars = vec![
            // Signal bar: O=16000 H=16050 L=15980 C=16010
            (16000.0, 16050.0, 15980.0, 16010.0),
            // Bar 1: drops below sell level (15978), triggering short
            (15990.0, 16000.0, 15970.0, 15975.0),
            // Bar 2: price reverses upward, hits SL at 16018
            (15980.0, 16025.0, 15975.0, 16020.0),
        ];

        let candles = make_day_candles(Instrument::Dax, d, &bars);

        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let sell_order = &orders[1]; // Short order
        assert_eq!(sell_order.trigger_price, dec!(15978));
        assert_eq!(sell_order.stop_loss, dec!(16018)); // 15978 + 40

        // Fill on bar 1
        let fill = check_fill(sell_order, &candles[1], dec!(0)).unwrap();
        assert_eq!(fill.fill_price, dec!(15978));

        let mut position = Position {
            direction: fill.direction,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: sell_order.stop_loss,
            size: config.position_size,
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        // Bar 2: high = 16025 >= SL (16018) -> SL hit
        let exit = position.update(&candles[2], &config).unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::StopLoss);
        assert_eq!(exit.exit_price, dec!(16018));

        let trade = position.close(exit, &config);
        assert_eq!(trade.pnl_points, dec!(-40)); // (16018 - 15978) * -1 = -40
    }

    /// Complete flow: Long trade with take profit hit.
    #[test]
    fn test_full_flow_long_take_profit() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::FixedTakeProfit,
            fixed_tp_points: dec!(80),
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let bars = vec![
            (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
            (16040.0, 16060.0, 16035.0, 16055.0), // fill candle
            (16060.0, 16100.0, 16050.0, 16090.0), // upward move
            (16090.0, 16140.0, 16080.0, 16135.0), // hits TP at 16132
        ];

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();

        let mut position = Position {
            direction: fill.direction,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: config.position_size,
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        // Bar 2: no exit (TP at 16132, high only 16100)
        assert!(position.update(&candles[2], &config).is_none());
        // Bar 3: TP hit (high 16140 >= 16132)
        let exit = position.update(&candles[3], &config).unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TakeProfit);
        assert_eq!(exit.exit_price, dec!(16132)); // 16052 + 80

        let trade = position.close(exit, &config);
        assert_eq!(trade.pnl_points, dec!(80));
    }

    /// Complete flow: Long trade with trailing stop.
    #[test]
    fn test_full_flow_long_trailing_stop() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::TrailingStop,
            trailing_stop_distance: dec!(20),
            trailing_stop_activation: dec!(0),
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let bars = vec![
            (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
            (16040.0, 16060.0, 16035.0, 16055.0), // fill candle
            (16060.0, 16090.0, 16075.0, 16085.0), // up, best=16090, trail=16070, low=16075 > 16070 -> no trigger
            (16085.0, 16095.0, 16060.0, 16070.0), // best=16095, trail=16075, low=16060 <= 16075 -> trail hit
        ];

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();

        let mut position = Position {
            direction: fill.direction,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: config.position_size,
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        // Bar 2: moves up, best_price updates to 16090, trail=16070, low=16075 -> safe
        assert!(position.update(&candles[2], &config).is_none());
        assert_eq!(position.best_price, dec!(16090));

        // Bar 3: best goes to 16095, trail = 16095 - 20 = 16075, low = 16060 <= 16075
        let exit = position.update(&candles[3], &config).unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TrailingStop);
        assert_eq!(exit.exit_price, dec!(16075)); // 16095 - 20

        let trade = position.close(exit, &config);
        assert_eq!(trade.pnl_points, dec!(23)); // 16075 - 16052
    }

    /// Complete flow: Long trade with adding to winners.
    #[test]
    fn test_full_flow_long_with_adds() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::EndOfDay,
            add_to_winners_enabled: true,
            add_every_points: dec!(30),
            max_additions: 2,
            add_size_multiplier: dec!(1),
            move_sl_on_add: true,
            add_sl_offset: dec!(5),
            position_size: dec!(1),
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        // Build enough bars to trigger 2 adds and then EOD
        let mut bars = vec![
            (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
            (16040.0, 16060.0, 16035.0, 16055.0), // fill candle
            // Price rises: add #1 at entry + 30 = 16082
            (16060.0, 16090.0, 16055.0, 16085.0),
            // Price rises more: add #2 at entry + 60 = 16112
            (16090.0, 16120.0, 16085.0, 16115.0),
        ];
        // Remaining bars up to EOD
        for _ in 4..33 {
            bars.push((16115.0, 16125.0, 16110.0, 16120.0));
        }
        // EOD bar at 16:30 UTC
        bars.push((16120.0, 16130.0, 16110.0, 16118.0));

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();

        let mut position = Position {
            direction: fill.direction,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: config.position_size,
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        let mut exit_result = None;
        for candle in &candles[2..] {
            if let Some(exit) = position.update(candle, &config) {
                exit_result = Some(exit);
                break;
            }
        }

        // Should have 2 adds
        assert_eq!(position.adds.len(), 2);
        // First add at 16082 (entry 16052 + 30)
        assert_eq!(position.adds[0].price, dec!(16082));
        // First add SL: reference=entry(16052), new SL = 16052 - 5 = 16047
        assert_eq!(position.adds[0].new_stop_loss, dec!(16047));
        // Second add at 16112 (entry 16052 + 60)
        assert_eq!(position.adds[1].price, dec!(16112));
        // Second add SL: reference=last_add(16082), new SL = 16082 - 5 = 16077
        assert_eq!(position.adds[1].new_stop_loss, dec!(16077));

        // Position SL should be tightened to last add's SL
        assert_eq!(position.stop_loss, dec!(16077));

        let exit = exit_result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::EndOfDay);

        let trade = position.close(exit, &config);
        // Base PnL: (16118 - 16052) * 1 = 66
        // Add1 PnL: (16118 - 16082) * 1 = 36
        // Add2 PnL: (16118 - 16112) * 1 = 6
        // Total: 66 + 36 + 6 = 108
        assert_eq!(trade.pnl_points, dec!(66));
        assert_eq!(trade.pnl_with_adds, dec!(108));
    }

    /// Both buy and sell orders trigger on same candle (wide-range bar).
    #[test]
    fn test_full_flow_both_sides_triggered() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::EndOfDay,
            allow_both_sides: true,
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let bars = vec![
            (16000.0, 16050.0, 15980.0, 16010.0), // signal bar
            // Wide bar: triggers both buy (16052) and sell (15978)
            (16015.0, 16060.0, 15970.0, 16000.0),
        ];

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);

        let fills = determine_fill_order(&orders[0], &orders[1], &candles[1], &config);
        assert_eq!(fills.len(), 2);

        // Open is 16015, buy trigger is 16052 (distance 37), sell trigger is 15978 (distance 37)
        // Equidistant -> buy fills first (deterministic tie-breaker)
        assert_eq!(fills[0].direction, Direction::Long);
        assert_eq!(fills[1].direction, Direction::Short);
    }

    /// No signal bar found on a day with no candles (holiday).
    #[test]
    fn test_full_flow_holiday_no_signal_bar() {
        let d = date(2024, 12, 25);
        let candles = Vec::new();
        let config = StrategyConfig::default();
        let result = find_signal_bar(&candles, Instrument::Dax, d, &config);
        assert!(result.is_none());
    }

    /// Order is not filled when price doesn't reach trigger.
    #[test]
    fn test_full_flow_order_not_filled() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig::default();

        let bars = vec![
            (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
            // Bar 1: price stays within signal bar range, no fill
            (16010.0, 16040.0, 15990.0, 16020.0),
        ];

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);

        // Neither buy (16052) nor sell (15978) triggers
        let buy_fill = check_fill(&orders[0], &candles[1], dec!(0));
        let sell_fill = check_fill(&orders[1], &candles[1], dec!(0));
        assert!(buy_fill.is_none());
        assert!(sell_fill.is_none());
    }

    /// Gap-up fill: open is above the buy trigger, so fill at open.
    #[test]
    fn test_full_flow_gap_up_fill() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let bars = vec![
            (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
            // Bar 1: gaps up above buy trigger 16052
            (16070.0, 16080.0, 16065.0, 16075.0),
        ];

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);

        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();
        assert_eq!(fill.fill_price, dec!(16070)); // fills at open, not trigger
    }

    // =========================================================================
    // Known-answer tests (hand-verified scenarios)
    // =========================================================================

    /// Known-answer: DAX long trade, fixed SL, EOD exit, no adds.
    /// Entry: 16052 (buy stop), SL: 16012 (fixed 40pt), EOD exit at 16100.
    /// Expected PnL: 48 points (16100 - 16052).
    #[test]
    fn test_known_answer_dax_long_no_adds() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::EndOfDay,
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let mut bars = Vec::new();
        bars.push((16000.0, 16050.0, 15980.0, 16030.0)); // signal bar
        bars.push((16040.0, 16060.0, 16035.0, 16055.0)); // fill
        // 31 bars of normal trading (no SL hit, SL at 16012)
        for _ in 0..31 {
            bars.push((16060.0, 16080.0, 16050.0, 16070.0));
        }
        // EOD bar (16:30 UTC = 17:30 CET winter)
        bars.push((16095.0, 16105.0, 16090.0, 16100.0));

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();

        let mut position = Position {
            direction: Direction::Long,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: dec!(1),
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        let mut exit_result = None;
        for candle in &candles[2..] {
            if let Some(exit) = position.update(candle, &config) {
                exit_result = Some(exit);
                break;
            }
        }

        let exit = exit_result.unwrap();
        let trade = position.close(exit, &config);

        assert_eq!(trade.entry_price, dec!(16052));
        assert_eq!(trade.exit_price, dec!(16100));
        assert_eq!(trade.pnl_points, dec!(48));
        assert_eq!(trade.pnl_with_adds, dec!(48));
        assert_eq!(trade.exit_reason, PositionStatus::EndOfDay);
        assert_eq!(trade.direction, Direction::Long);
        assert!(trade.adds.is_empty());
    }

    /// Known-answer: Short trade with commission and slippage.
    /// Entry: 15978 (sell stop), SL: 16018 (fixed 40pt).
    /// Price drops to 15900 at EOD.
    /// PnL raw: (15978 - 15900) = 78 points.
    /// With slippage (0.5 * 1 * 2 = 1.0) and commission (5 * 1 = 5): 78 - 1 - 5 = 72.
    #[test]
    fn test_known_answer_short_with_costs() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::EndOfDay,
            commission_per_trade: dec!(5),
            slippage_points: dec!(0.5),
            ..StrategyConfig::default()
        };

        let mut bars = Vec::new();
        bars.push((16000.0, 16050.0, 15980.0, 16010.0)); // signal bar
        bars.push((15990.0, 16000.0, 15970.0, 15975.0)); // fill (sell stop triggers)
        // Downtrend bars
        for _ in 0..31 {
            bars.push((15950.0, 15960.0, 15940.0, 15945.0));
        }
        // EOD
        bars.push((15910.0, 15920.0, 15895.0, 15900.0));

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let sell_order = &orders[1];
        let fill = check_fill(sell_order, &candles[1], config.slippage_points).unwrap();

        // Fill price = min(15978, 15990) - 0.5 = 15978 - 0.5 = 15977.5
        assert_eq!(fill.fill_price, dec!(15977.5));

        let mut position = Position {
            direction: Direction::Short,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: sell_order.stop_loss,
            size: dec!(1),
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        let mut exit_result = None;
        for candle in &candles[2..] {
            if let Some(exit) = position.update(candle, &config) {
                exit_result = Some(exit);
                break;
            }
        }

        let exit = exit_result.unwrap();
        let trade = position.close(exit, &config);

        assert_eq!(trade.exit_reason, PositionStatus::EndOfDay);
        // Raw PnL: (15900 - 15977.5) * -1 = 77.5
        assert_eq!(trade.pnl_points, dec!(77.5));
        // With costs: 77.5 - 5 (commission) - 1 (slippage: 0.5 * 1 * 2) = 71.5
        assert_eq!(trade.pnl_with_adds, dec!(71.5));
    }

    /// Known-answer: Signal bar extreme SL mode.
    /// Long SL should be at signal bar low, short SL at signal bar high.
    #[test]
    fn test_known_answer_signal_bar_extreme_sl() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            sl_mode: StopLossMode::SignalBarExtreme,
            ..StrategyConfig::default()
        };

        let bars = vec![(16000.0, 16050.0, 15980.0, 16030.0)];
        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);

        // Long SL = signal bar low = 15980
        assert_eq!(orders[0].stop_loss, dec!(15980));
        // Short SL = signal bar high = 16050
        assert_eq!(orders[1].stop_loss, dec!(16050));
    }

    /// Known-answer: Midpoint SL mode.
    /// Midpoint = (16050 + 15980) / 2 = 16015.
    /// Long SL = 16015 - 5 = 16010. Short SL = 16015 + 5 = 16020.
    #[test]
    fn test_known_answer_midpoint_sl() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            sl_mode: StopLossMode::Midpoint,
            sl_midpoint_offset: dec!(5),
            ..StrategyConfig::default()
        };

        let bars = vec![(16000.0, 16050.0, 15980.0, 16030.0)];
        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);

        assert_eq!(orders[0].stop_loss, dec!(16010)); // 16015 - 5
        assert_eq!(orders[1].stop_loss, dec!(16020)); // 16015 + 5
    }

    /// Known-answer: FTSE summer day, full flow.
    #[test]
    fn test_known_answer_ftse_summer_long() {
        let d = date(2024, 7, 15); // BST (UTC+1)
        let config = StrategyConfig {
            instrument: Instrument::Ftse,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(30),
            exit_mode: ExitMode::FixedTakeProfit,
            fixed_tp_points: dec!(50),
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let bars = vec![
            (7500.0, 7530.0, 7480.0, 7520.0), // signal bar at 07:15 UTC
            (7525.0, 7540.0, 7520.0, 7535.0), // fill (buy at 7532)
            (7535.0, 7585.0, 7530.0, 7580.0), // TP at 7582 hit (high 7585)
        ];

        let candles = make_day_candles(Instrument::Ftse, d, &bars);

        // Verify signal bar time (07:15 UTC in summer)
        let signal_bar = find_signal_bar(&candles, Instrument::Ftse, d, &config).unwrap();
        assert_eq!(
            signal_bar.candle.timestamp.format("%H:%M").to_string(),
            "07:15"
        );
        assert_eq!(signal_bar.buy_level, dec!(7532)); // 7530 + 2
        assert_eq!(signal_bar.sell_level, dec!(7478)); // 7480 - 2

        let orders = generate_orders(&signal_bar, &config);
        assert_eq!(orders[0].stop_loss, dec!(7502)); // 7532 - 30
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();
        assert_eq!(fill.fill_price, dec!(7532));

        let mut position = Position {
            direction: Direction::Long,
            entry_price: dec!(7532),
            entry_time: fill.fill_time,
            stop_loss: dec!(7502),
            size: dec!(1),
            best_price: dec!(7532),
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        let exit = position.update(&candles[2], &config).unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TakeProfit);
        assert_eq!(exit.exit_price, dec!(7582)); // 7532 + 50

        let trade = position.close(exit, &config);
        assert_eq!(trade.pnl_points, dec!(50));
    }

    /// Known-answer: Conservative SL-first assumption.
    /// When both SL and TP could trigger on the same candle, SL wins.
    #[test]
    fn test_known_answer_sl_before_tp_same_candle() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::FixedTakeProfit,
            fixed_tp_points: dec!(50),
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let bars = vec![
            (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
            (16040.0, 16060.0, 16035.0, 16055.0), // fill at 16052
            // Wide bar: both SL (16012) and TP (16102) could trigger
            (16050.0, 16110.0, 16005.0, 16080.0),
        ];

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();

        let mut position = Position {
            direction: Direction::Long,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: dec!(1),
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        // SL at 16012, TP at 16102. Candle low=16005 <= 16012 -> SL first (conservative)
        let exit = position.update(&candles[2], &config).unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::StopLoss);
        assert_eq!(exit.exit_price, dec!(16012));

        let trade = position.close(exit, &config);
        assert_eq!(trade.pnl_points, dec!(-40)); // 16012 - 16052
    }

    /// Known-answer: Adding to winners with 3 adds, then SL hit.
    #[test]
    fn test_known_answer_three_adds_then_sl() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::EndOfDay,
            add_to_winners_enabled: true,
            add_every_points: dec!(20),
            max_additions: 3,
            add_size_multiplier: dec!(1),
            move_sl_on_add: true,
            add_sl_offset: dec!(0),
            position_size: dec!(1),
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let bars = vec![
            (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
            (16040.0, 16060.0, 16035.0, 16055.0), // fill at 16052
            // Add1 at 16072 (entry+20), high needs >= 16072
            (16060.0, 16080.0, 16055.0, 16075.0),
            // Add2 at 16092 (entry+40), high needs >= 16092
            (16080.0, 16100.0, 16075.0, 16095.0),
            // Add3 at 16112 (entry+60), high needs >= 16112
            (16100.0, 16120.0, 16095.0, 16115.0),
            // Price reverses, hits tightened SL
            // After add3: SL = add2 price (16092) - 0 = 16092
            (16100.0, 16105.0, 16085.0, 16090.0),
        ];

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();

        let mut position = Position {
            direction: Direction::Long,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: dec!(1),
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        // Process bars 2-4: should add 3 times
        assert!(position.update(&candles[2], &config).is_none());
        assert_eq!(position.adds.len(), 1);
        assert_eq!(position.adds[0].price, dec!(16072));

        assert!(position.update(&candles[3], &config).is_none());
        assert_eq!(position.adds.len(), 2);
        assert_eq!(position.adds[1].price, dec!(16092));

        assert!(position.update(&candles[4], &config).is_none());
        assert_eq!(position.adds.len(), 3);
        assert_eq!(position.adds[2].price, dec!(16112));

        // SL should now be at add2's price (16092) with offset 0
        assert_eq!(position.stop_loss, dec!(16092));

        // Bar 5: low = 16085 <= SL (16092) -> SL hit
        let exit = position.update(&candles[5], &config).unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::StopLoss);
        assert_eq!(exit.exit_price, dec!(16092));

        let trade = position.close(exit, &config);
        // Base: (16092 - 16052) * 1 = 40
        assert_eq!(trade.pnl_points, dec!(40));
        // Add1: (16092 - 16072) * 1 = 20
        // Add2: (16092 - 16092) * 1 = 0
        // Add3: (16092 - 16112) * 1 = -20
        // Total: 40 + 20 + 0 + (-20) = 40
        assert_eq!(trade.pnl_with_adds, dec!(40));
    }

    /// Verify US instrument (Nasdaq) works end-to-end with correct timezone handling.
    #[test]
    fn test_known_answer_nasdaq_winter_full_flow() {
        let d = date(2024, 1, 15); // EST (UTC-5)
        let config = StrategyConfig {
            instrument: Instrument::Nasdaq,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(50),
            exit_mode: ExitMode::FixedTakeProfit,
            fixed_tp_points: dec!(40),
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let bars = vec![
            (15000.0, 15050.0, 14950.0, 15020.0), // signal bar at 14:45 UTC
            (15030.0, 15060.0, 15020.0, 15055.0), // fill (buy at 15052)
            (15055.0, 15100.0, 15050.0, 15095.0), // TP at 15092 hit
        ];

        let candles = make_day_candles(Instrument::Nasdaq, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Nasdaq, d, &config).unwrap();

        // Verify signal bar at 14:45 UTC (09:45 EST in winter)
        assert_eq!(
            signal_bar.candle.timestamp.format("%H:%M").to_string(),
            "14:45"
        );

        let orders = generate_orders(&signal_bar, &config);
        assert_eq!(orders[0].trigger_price, dec!(15052)); // 15050 + 2
        assert_eq!(orders[0].stop_loss, dec!(15002)); // 15052 - 50

        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();
        let mut position = Position {
            direction: Direction::Long,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: dec!(1),
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        let exit = position.update(&candles[2], &config).unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TakeProfit);
        assert_eq!(exit.exit_price, dec!(15092)); // 15052 + 40

        let trade = position.close(exit, &config);
        assert_eq!(trade.pnl_points, dec!(40));
        assert_eq!(trade.instrument, Instrument::Nasdaq);
    }

    /// Verify adding check is independent of trailing stop.
    #[test]
    fn test_adding_does_not_interfere_with_trailing_stop() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::TrailingStop,
            trailing_stop_distance: dec!(25),
            trailing_stop_activation: dec!(10),
            add_to_winners_enabled: true,
            add_every_points: dec!(30),
            max_additions: 1,
            add_size_multiplier: dec!(1),
            move_sl_on_add: true,
            add_sl_offset: dec!(0),
            position_size: dec!(1),
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let bars = vec![
            (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
            (16040.0, 16060.0, 16035.0, 16055.0), // fill at 16052
            // Price rises past add trigger (16082), best=16090, trail=16065, low=16070 -> safe
            (16070.0, 16090.0, 16070.0, 16085.0),
            // Trail: best stays 16090 (high 16085 < 16090), trail=16065, low=16060 <= 16065 -> hit
            (16080.0, 16085.0, 16060.0, 16065.0),
        ];

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();

        let mut position = Position {
            direction: Direction::Long,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: dec!(1),
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        // Bar 2: should trigger add at 16082 and update best_price
        assert!(position.update(&candles[2], &config).is_none());
        assert_eq!(position.adds.len(), 1);
        assert_eq!(position.best_price, dec!(16090));

        // Bar 3: trailing stop triggers
        // best stays 16090 (high=16085 < 16090), trail = 16090 - 25 = 16065, low = 16060 <= 16065
        let exit = position.update(&candles[3], &config).unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TrailingStop);
        assert_eq!(exit.exit_price, dec!(16065));

        let trade = position.close(exit, &config);
        // Base: (16065 - 16052) * 1 = 13
        assert_eq!(trade.pnl_points, dec!(13));
        // Add: (16065 - 16082) * 1 = -17
        // Total: 13 + (-17) = -4
        assert_eq!(trade.pnl_with_adds, dec!(-4));
    }

    // =========================================================================
    // Edge case tests
    // =========================================================================

    /// Flat signal bar (high == low): both buy and sell triggers are offset from same level.
    #[test]
    fn test_edge_case_flat_signal_bar() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            sl_mode: StopLossMode::SignalBarExtreme,
            ..StrategyConfig::default()
        };

        let bars = vec![(16000.0, 16000.0, 16000.0, 16000.0)]; // flat
        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();

        assert_eq!(signal_bar.buy_level, dec!(16002));
        assert_eq!(signal_bar.sell_level, dec!(15998));

        let orders = generate_orders(&signal_bar, &config);
        // Both SLs at signal bar extreme (which is 16000 for both)
        assert_eq!(orders[0].stop_loss, dec!(16000)); // Long SL = signal bar low
        assert_eq!(orders[1].stop_loss, dec!(16000)); // Short SL = signal bar high
    }

    /// Position with add_size_multiplier > 1 computes weighted PnL correctly.
    #[test]
    fn test_edge_case_double_size_adds() {
        let d = date(2024, 1, 15);
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            exit_mode: ExitMode::EndOfDay,
            add_to_winners_enabled: true,
            add_every_points: dec!(30),
            max_additions: 1,
            add_size_multiplier: dec!(2),
            move_sl_on_add: false,
            position_size: dec!(1),
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        let mut bars = Vec::new();
        bars.push((16000.0, 16050.0, 15980.0, 16030.0)); // signal bar
        bars.push((16040.0, 16060.0, 16035.0, 16055.0)); // fill at 16052
        bars.push((16060.0, 16090.0, 16055.0, 16085.0)); // add at 16082
        // Fill remaining bars to EOD
        for _ in 0..31 {
            bars.push((16085.0, 16095.0, 16080.0, 16090.0));
        }
        bars.push((16090.0, 16100.0, 16085.0, 16092.0)); // EOD

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();

        let mut position = Position {
            direction: Direction::Long,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: dec!(1),
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        let mut exit_result = None;
        for candle in &candles[2..] {
            if let Some(exit) = position.update(candle, &config) {
                exit_result = Some(exit);
                break;
            }
        }

        assert_eq!(position.adds.len(), 1);
        assert_eq!(position.adds[0].size, dec!(2)); // double size

        let exit = exit_result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::EndOfDay);
        let trade = position.close(exit, &config);
        // EOD triggers at bar 33 (16:30 UTC = 17:30 CET), close = 16090
        // Base: (16090 - 16052) * 1 = 38
        // Add: (16090 - 16082) * 1 = 8, weighted by size 2 = 16
        // Total: 38 + 16 = 54
        assert_eq!(trade.pnl_points, dec!(38));
        assert_eq!(trade.pnl_with_adds, dec!(54));
    }

    /// Verify CloseAtTime mode works across timezone boundary.
    #[test]
    fn test_edge_case_close_at_time_dax_summer() {
        let d = date(2024, 7, 15); // CEST (UTC+2)
        let config = StrategyConfig {
            instrument: Instrument::Dax,
            exit_mode: ExitMode::CloseAtTime,
            close_at_time: chrono::NaiveTime::from_hms_opt(14, 0, 0).unwrap(), // 14:00 CEST
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        };

        // 14:00 CEST = 12:00 UTC in summer
        let mut bars = Vec::new();
        bars.push((18000.0, 18050.0, 17980.0, 18030.0)); // signal bar at 07:15 UTC
        bars.push((18040.0, 18060.0, 18035.0, 18055.0)); // fill
        // Bars until 12:00 UTC (14:00 CEST): that's (12:00 - 07:30) / 15 = 18 bars
        for _ in 0..18 {
            bars.push((18060.0, 18070.0, 18050.0, 18065.0));
        }
        // At 12:00 UTC = 14:00 CEST: close at time
        bars.push((18065.0, 18075.0, 18060.0, 18070.0));

        let candles = make_day_candles(Instrument::Dax, d, &bars);
        let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&signal_bar, &config);
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();

        let mut position = Position {
            direction: Direction::Long,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: dec!(1),
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        let mut exit_result = None;
        for candle in &candles[2..] {
            if let Some(exit) = position.update(candle, &config) {
                exit_result = Some(exit);
                break;
            }
        }

        let exit = exit_result.unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::TimeClose);
    }
}
