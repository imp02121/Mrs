//! Comprehensive strategy tests: known-answer tests, full integration flows,
//! and edge cases that span multiple strategy modules.
//!
//! These tests exercise the complete pipeline:
//! candles -> signal bar -> orders -> fills -> position updates -> trade close.

use rust_decimal_macros::dec;

use crate::models::Instrument;
use crate::strategy::config::StrategyConfig;
use crate::strategy::fill::{check_fill, determine_fill_order};
use crate::strategy::order::generate_orders;
use crate::strategy::position::Position;
use crate::strategy::signal::find_signal_bar;
use crate::strategy::types::{Direction, ExitMode, PositionStatus, StopLossMode};
use crate::test_helpers::{date, default_config, make_candle, make_day_candles, make_signal_bar};

// ============================================================================
// Full integration: candles -> signal bar -> orders -> fill -> position -> trade
// ============================================================================

#[test]
fn test_full_flow_dax_long_winning_trade() {
    // DAX winter date: signal bar at 08:15 UTC
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        sl_mode: StopLossMode::FixedPoints,
        sl_fixed_points: dec!(40),
        exit_mode: ExitMode::EndOfDay,
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    // Build candles: signal bar + subsequent bars
    // Signal bar: O=16000 H=16050 L=15980 C=16030
    // Bar 2: price rises, buy stop fills
    // Bar 3: price keeps rising
    // Bar 4: EOD bar (17:30 CET = 16:30 UTC)
    let candles = make_day_candles(
        Instrument::Dax,
        d,
        &[
            (16000.0, 16050.0, 15980.0, 16030.0), // signal bar at 08:15
            (16040.0, 16060.0, 16020.0, 16055.0), // 08:30 - buy stop fills
            (16055.0, 16100.0, 16040.0, 16090.0), // 08:45 - price rises
        ],
    );

    // 1. Find signal bar
    let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
    assert_eq!(signal_bar.buy_level, dec!(16052)); // 16050 + 2
    assert_eq!(signal_bar.sell_level, dec!(15978)); // 15980 - 2

    // 2. Generate orders
    let orders = generate_orders(&signal_bar, &config);
    assert_eq!(orders.len(), 2);
    let buy_order = &orders[0];
    assert_eq!(buy_order.direction, Direction::Long);
    assert_eq!(buy_order.trigger_price, dec!(16052));
    assert_eq!(buy_order.stop_loss, dec!(16012)); // 16052 - 40

    let sell_order = &orders[1];
    assert_eq!(sell_order.direction, Direction::Short);
    assert_eq!(sell_order.trigger_price, dec!(15978));

    // 3. Check fills on bar 2 (08:30 candle)
    let bar2 = &candles[1];
    let buy_fill = check_fill(buy_order, bar2, config.slippage_points);
    assert!(buy_fill.is_some());
    let fill = buy_fill.unwrap();
    assert_eq!(fill.fill_price, dec!(16052));
    assert_eq!(fill.direction, Direction::Long);

    // Sell order should NOT fill on bar 2 (low=16020 > trigger=15978)
    let sell_fill = check_fill(sell_order, bar2, config.slippage_points);
    assert!(sell_fill.is_none());

    // 4. Create position from fill
    let mut position = Position {
        direction: fill.direction,
        entry_price: fill.fill_price,
        entry_time: fill.fill_time,
        stop_loss: fill.order.stop_loss,
        size: fill.order.size,
        best_price: fill.fill_price,
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    // 5. Update position with bar 3 (08:45) - no exit
    let bar3 = &candles[2];
    let exit = position.update(bar3, &config);
    assert!(exit.is_none());
    assert_eq!(position.best_price, dec!(16100)); // high of bar 3

    // 6. Create EOD candle (16:30 UTC = 17:30 CET in winter)
    let eod_candle = make_candle(
        Instrument::Dax,
        "2024-01-15 16:30",
        "16080",
        "16095",
        "16070",
        "16085",
    );
    let exit = position.update(&eod_candle, &config).unwrap();
    assert_eq!(exit.exit_reason, PositionStatus::EndOfDay);
    assert_eq!(exit.exit_price, dec!(16085)); // closes at candle.close

    // 7. Close position -> Trade
    let position_for_close = Position {
        direction: Direction::Long,
        entry_price: dec!(16052),
        entry_time: fill.fill_time,
        stop_loss: dec!(16012),
        size: dec!(1),
        best_price: dec!(16100),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };
    let trade = position_for_close.close(exit, &config);
    assert_eq!(trade.direction, Direction::Long);
    assert_eq!(trade.entry_price, dec!(16052));
    assert_eq!(trade.exit_price, dec!(16085));
    assert_eq!(trade.pnl_points, dec!(33)); // 16085 - 16052
    assert_eq!(trade.exit_reason, PositionStatus::EndOfDay);
}

#[test]
fn test_full_flow_dax_short_stop_loss() {
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        sl_mode: StopLossMode::FixedPoints,
        sl_fixed_points: dec!(40),
        exit_mode: ExitMode::EndOfDay,
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    let candles = make_day_candles(
        Instrument::Dax,
        d,
        &[
            (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
            (15990.0, 16000.0, 15970.0, 15975.0), // sell stop fills
            (15980.0, 16025.0, 15970.0, 16020.0), // price reverses, SL hit
        ],
    );

    let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
    let orders = generate_orders(&signal_bar, &config);
    let sell_order = &orders[1];
    assert_eq!(sell_order.trigger_price, dec!(15978));
    assert_eq!(sell_order.stop_loss, dec!(16018)); // 15978 + 40

    // Fill on bar 2
    let bar2 = &candles[1];
    let fill = check_fill(sell_order, bar2, dec!(0)).unwrap();
    assert_eq!(fill.fill_price, dec!(15978));
    assert_eq!(fill.direction, Direction::Short);

    // Create position
    let mut position = Position {
        direction: Direction::Short,
        entry_price: dec!(15978),
        entry_time: fill.fill_time,
        stop_loss: dec!(16018),
        size: dec!(1),
        best_price: dec!(15978),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    // Bar 3: high = 16025 >= SL at 16018 -> SL hit
    let bar3 = &candles[2];
    let exit = position.update(bar3, &config).unwrap();
    assert_eq!(exit.exit_reason, PositionStatus::StopLoss);
    assert_eq!(exit.exit_price, dec!(16018));

    let trade = Position {
        direction: Direction::Short,
        entry_price: dec!(15978),
        entry_time: fill.fill_time,
        stop_loss: dec!(16018),
        size: dec!(1),
        best_price: dec!(15978),
        adds: Vec::new(),
        status: PositionStatus::Open,
    }
    .close(exit, &config);
    assert_eq!(trade.pnl_points, dec!(-40)); // (16018 - 15978) * -1 = -40
}

#[test]
fn test_full_flow_both_sides_triggered_same_session() {
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        sl_mode: StopLossMode::FixedPoints,
        sl_fixed_points: dec!(40),
        exit_mode: ExitMode::EndOfDay,
        allow_both_sides: true,
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    // Signal bar with tight range
    let candles = make_day_candles(
        Instrument::Dax,
        d,
        &[
            (16000.0, 16010.0, 15990.0, 16005.0), // signal bar (20pt range)
        ],
    );

    let signal_bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
    assert_eq!(signal_bar.buy_level, dec!(16012)); // 16010 + 2
    assert_eq!(signal_bar.sell_level, dec!(15988)); // 15990 - 2

    let orders = generate_orders(&signal_bar, &config);

    // Wide candle that triggers both sides: open near middle
    let wide_candle = make_candle(
        Instrument::Dax,
        "2024-01-15 08:30",
        "16000",
        "16020",
        "15980",
        "16010",
    );

    let fills = determine_fill_order(&orders[0], &orders[1], &wide_candle, &config);
    assert_eq!(fills.len(), 2);

    // Open=16000, buy_trigger=16012 (dist=12), sell_trigger=15988 (dist=12)
    // Equidistant -> buy fills first
    assert_eq!(fills[0].direction, Direction::Long);
    assert_eq!(fills[1].direction, Direction::Short);
}

// ============================================================================
// Known-answer tests: hand-crafted scenarios with exact expected values
// ============================================================================

#[test]
fn test_known_answer_dax_winter_signal_bar_extreme_sl() {
    // Scenario: DAX, Jan 15 2024 (winter CET), SignalBarExtreme SL mode
    // Signal bar: H=16100 L=16000, offset=2
    // Buy trigger = 16102, Buy SL = 16000 (signal bar low)
    // Sell trigger = 15998, Sell SL = 16100 (signal bar high)
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        sl_mode: StopLossMode::SignalBarExtreme,
        entry_offset_points: dec!(2),
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    let bar = make_signal_bar(
        Instrument::Dax,
        d,
        16050.0,
        16100.0,
        16000.0,
        16080.0,
        &config,
    );
    let orders = generate_orders(&bar, &config);

    let buy = &orders[0];
    assert_eq!(buy.trigger_price, dec!(16102));
    assert_eq!(buy.stop_loss, dec!(16000));

    let sell = &orders[1];
    assert_eq!(sell.trigger_price, dec!(15998));
    assert_eq!(sell.stop_loss, dec!(16100));

    // Buy fills, price runs to 16200, then closes at EOD
    let fill_candle = make_candle(
        Instrument::Dax,
        "2024-01-15 08:30",
        "16090",
        "16110",
        "16080",
        "16105",
    );
    let fill = check_fill(buy, &fill_candle, dec!(0)).unwrap();
    assert_eq!(fill.fill_price, dec!(16102));

    let position = Position {
        direction: Direction::Long,
        entry_price: dec!(16102),
        entry_time: fill.fill_time,
        stop_loss: dec!(16000),
        size: dec!(1),
        best_price: dec!(16102),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    // EOD candle
    let eod = make_candle(
        Instrument::Dax,
        "2024-01-15 16:30",
        "16180",
        "16200",
        "16170",
        "16190",
    );
    let mut pos = position;
    let exit = pos.update(&eod, &config).unwrap();
    assert_eq!(exit.exit_reason, PositionStatus::EndOfDay);
    assert_eq!(exit.exit_price, dec!(16190));

    let trade = Position {
        direction: Direction::Long,
        entry_price: dec!(16102),
        entry_time: fill.fill_time,
        stop_loss: dec!(16000),
        size: dec!(1),
        best_price: dec!(16200),
        adds: Vec::new(),
        status: PositionStatus::Open,
    }
    .close(exit, &config);
    assert_eq!(trade.pnl_points, dec!(88)); // 16190 - 16102
}

#[test]
fn test_known_answer_midpoint_sl_calculation() {
    // Signal bar: H=16100 L=16000, midpoint=16050
    // Midpoint SL with offset=5:
    //   Long SL = 16050 - 5 = 16045
    //   Short SL = 16050 + 5 = 16055
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        sl_mode: StopLossMode::Midpoint,
        sl_midpoint_offset: dec!(5),
        entry_offset_points: dec!(2),
        slippage_points: dec!(0),
        ..default_config()
    };

    let bar = make_signal_bar(
        Instrument::Dax,
        d,
        16050.0,
        16100.0,
        16000.0,
        16080.0,
        &config,
    );
    let orders = generate_orders(&bar, &config);

    assert_eq!(orders[0].trigger_price, dec!(16102));
    assert_eq!(orders[0].stop_loss, dec!(16045)); // midpoint(16050) - 5
    assert_eq!(orders[1].trigger_price, dec!(15998));
    assert_eq!(orders[1].stop_loss, dec!(16055)); // midpoint(16050) + 5
}

#[test]
fn test_known_answer_scaled_fixed_sl() {
    // sl_fixed_points=40 calibrated at 12000. Current price=18000.
    // Scaled SL = 40 * 18000/12000 = 60
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        sl_mode: StopLossMode::FixedPoints,
        sl_fixed_points: dec!(40),
        sl_scale_with_index: true,
        sl_scale_baseline: dec!(12000),
        entry_offset_points: dec!(2),
        slippage_points: dec!(0),
        ..default_config()
    };

    let bar = make_signal_bar(
        Instrument::Dax,
        d,
        18050.0,
        18100.0,
        18000.0,
        18000.0,
        &config,
    );
    let orders = generate_orders(&bar, &config);

    // scaled = 40 * 18000 / 12000 = 60
    // buy trigger = 18102, buy SL = 18102 - 60 = 18042
    assert_eq!(orders[0].trigger_price, dec!(18102));
    assert_eq!(orders[0].stop_loss, dec!(18042));

    // sell trigger = 17998, sell SL = 17998 + 60 = 18058
    assert_eq!(orders[1].trigger_price, dec!(17998));
    assert_eq!(orders[1].stop_loss, dec!(18058));
}

// ============================================================================
// Full flow with adding to winners
// ============================================================================

#[test]
fn test_full_flow_with_adds_and_trailing_stop() {
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        sl_mode: StopLossMode::FixedPoints,
        sl_fixed_points: dec!(40),
        exit_mode: ExitMode::TrailingStop,
        trailing_stop_distance: dec!(30),
        trailing_stop_activation: dec!(0),
        add_to_winners_enabled: true,
        add_every_points: dec!(50),
        max_additions: 2,
        add_size_multiplier: dec!(1),
        move_sl_on_add: true,
        add_sl_offset: dec!(5),
        position_size: dec!(1),
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    // Signal bar
    let bar = make_signal_bar(
        Instrument::Dax,
        d,
        16000.0,
        16050.0,
        15980.0,
        16030.0,
        &config,
    );
    let orders = generate_orders(&bar, &config);
    let buy_order = &orders[0];
    assert_eq!(buy_order.trigger_price, dec!(16052));

    // Fill candle
    let fill_candle = make_candle(
        Instrument::Dax,
        "2024-01-15 08:30",
        "16040",
        "16060",
        "16035",
        "16055",
    );
    let fill = check_fill(buy_order, &fill_candle, dec!(0)).unwrap();
    assert_eq!(fill.fill_price, dec!(16052));

    let mut position = Position {
        direction: Direction::Long,
        entry_price: dec!(16052),
        entry_time: fill.fill_time,
        stop_loss: dec!(16012),
        size: dec!(1),
        best_price: dec!(16052),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    // Bar that triggers first add: entry + 50 = 16102
    let add_candle_1 = make_candle(
        Instrument::Dax,
        "2024-01-15 09:00",
        "16090",
        "16110",
        "16085",
        "16105",
    );
    let exit = position.update(&add_candle_1, &config);
    assert!(exit.is_none());
    assert_eq!(position.adds.len(), 1);
    assert_eq!(position.adds[0].price, dec!(16102));
    // SL tightened: reference=entry(16052), new SL = 16052 - 5 = 16047
    assert_eq!(position.stop_loss, dec!(16047));

    // Bar that triggers second add: entry + 100 = 16152
    let add_candle_2 = make_candle(
        Instrument::Dax,
        "2024-01-15 09:15",
        "16140",
        "16160",
        "16135",
        "16155",
    );
    let exit = position.update(&add_candle_2, &config);
    assert!(exit.is_none());
    assert_eq!(position.adds.len(), 2);
    assert_eq!(position.adds[1].price, dec!(16152));
    // SL tightened: reference=last add(16102), new SL = 16102 - 5 = 16097
    assert_eq!(position.stop_loss, dec!(16097));
    assert_eq!(position.best_price, dec!(16160));

    // Bar where trailing stop is hit: best=16160, trail=16160-30=16130
    // Candle low = 16125 <= 16130 -> trailing stop hit
    let trailing_candle = make_candle(
        Instrument::Dax,
        "2024-01-15 09:30",
        "16150",
        "16155",
        "16125",
        "16130",
    );
    let exit = position.update(&trailing_candle, &config).unwrap();
    assert_eq!(exit.exit_reason, PositionStatus::TrailingStop);
    assert_eq!(exit.exit_price, dec!(16130));

    // Close and compute PnL
    let trade = Position {
        direction: Direction::Long,
        entry_price: dec!(16052),
        entry_time: fill.fill_time,
        stop_loss: dec!(16097),
        size: dec!(1),
        best_price: dec!(16160),
        adds: position.adds.clone(),
        status: PositionStatus::Open,
    }
    .close(exit, &config);

    assert_eq!(trade.pnl_points, dec!(78)); // 16130 - 16052
    assert_eq!(trade.adds.len(), 2);
    // Add1 PnL: (16130 - 16102) * 1 = 28
    assert_eq!(trade.adds[0].pnl_points, dec!(28));
    // Add2 PnL: (16130 - 16152) * 1 = -22
    assert_eq!(trade.adds[1].pnl_points, dec!(-22));
    // Total = base(78*1) + add1(28*1) + add2(-22*1) = 78 + 28 - 22 = 84
    assert_eq!(trade.pnl_with_adds, dec!(84));
}

// ============================================================================
// Gap fill scenarios
// ============================================================================

#[test]
fn test_gap_up_fill_price_at_open() {
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        slippage_points: dec!(0.5),
        ..default_config()
    };

    let bar = make_signal_bar(
        Instrument::Dax,
        d,
        16000.0,
        16050.0,
        15980.0,
        16030.0,
        &config,
    );
    let orders = generate_orders(&bar, &config);
    let buy_order = &orders[0];

    // Gap up: open at 16070, well above buy trigger 16052
    let gap_candle = make_candle(
        Instrument::Dax,
        "2024-01-15 08:30",
        "16070",
        "16090",
        "16065",
        "16080",
    );
    let fill = check_fill(buy_order, &gap_candle, config.slippage_points).unwrap();
    // Fill at open + slippage: 16070 + 0.5 = 16070.5
    assert_eq!(fill.fill_price, dec!(16070.5));
}

#[test]
fn test_gap_down_fill_price_at_open() {
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        slippage_points: dec!(0.5),
        ..default_config()
    };

    let bar = make_signal_bar(
        Instrument::Dax,
        d,
        16000.0,
        16050.0,
        15980.0,
        16030.0,
        &config,
    );
    let orders = generate_orders(&bar, &config);
    let sell_order = &orders[1];

    // Gap down: open at 15960, below sell trigger 15978
    let gap_candle = make_candle(
        Instrument::Dax,
        "2024-01-15 08:30",
        "15960",
        "15970",
        "15950",
        "15965",
    );
    let fill = check_fill(sell_order, &gap_candle, config.slippage_points).unwrap();
    // Fill at open - slippage: 15960 - 0.5 = 15959.5
    assert_eq!(fill.fill_price, dec!(15959.5));
}

// ============================================================================
// Signal bar detection across all instruments and DST
// ============================================================================

#[test]
fn test_signal_bar_all_instruments_winter() {
    let d = date(2024, 1, 15);
    let config = default_config();

    for instrument in Instrument::ALL {
        let candles = make_day_candles(instrument, d, &[(100.0, 110.0, 90.0, 105.0)]);
        let result = find_signal_bar(&candles, instrument, d, &config);
        assert!(
            result.is_some(),
            "signal bar not found for {instrument} on winter date"
        );
    }
}

#[test]
fn test_signal_bar_all_instruments_summer() {
    let d = date(2024, 7, 15);
    let config = default_config();

    for instrument in Instrument::ALL {
        let candles = make_day_candles(instrument, d, &[(100.0, 110.0, 90.0, 105.0)]);
        let result = find_signal_bar(&candles, instrument, d, &config);
        assert!(
            result.is_some(),
            "signal bar not found for {instrument} on summer date"
        );
    }
}

#[test]
fn test_signal_bar_ftse_dst_transition_dates() {
    let config = default_config();

    // Spring forward: 2024-03-31
    let spring = date(2024, 3, 31);
    let candles = make_day_candles(Instrument::Ftse, spring, &[(100.0, 110.0, 90.0, 105.0)]);
    let bar = find_signal_bar(&candles, Instrument::Ftse, spring, &config);
    assert!(bar.is_some());
    // 08:15 BST = 07:15 UTC on DST transition day
    assert_eq!(
        bar.unwrap().candle.timestamp.format("%H:%M").to_string(),
        "07:15"
    );

    // Fall back: 2024-10-27
    let autumn = date(2024, 10, 27);
    let candles = make_day_candles(Instrument::Ftse, autumn, &[(100.0, 110.0, 90.0, 105.0)]);
    let bar = find_signal_bar(&candles, Instrument::Ftse, autumn, &config);
    assert!(bar.is_some());
    // 08:15 GMT = 08:15 UTC after fall back
    assert_eq!(
        bar.unwrap().candle.timestamp.format("%H:%M").to_string(),
        "08:15"
    );
}

#[test]
fn test_signal_bar_us_dst_transition_dates() {
    let config = default_config();

    // US spring forward: 2024-03-10
    let spring = date(2024, 3, 10);
    let candles = make_day_candles(Instrument::Nasdaq, spring, &[(100.0, 110.0, 90.0, 105.0)]);
    let bar = find_signal_bar(&candles, Instrument::Nasdaq, spring, &config);
    assert!(bar.is_some());
    // 09:45 EDT = 13:45 UTC
    assert_eq!(
        bar.unwrap().candle.timestamp.format("%H:%M").to_string(),
        "13:45"
    );

    // US fall back: 2024-11-03
    let autumn = date(2024, 11, 3);
    let candles = make_day_candles(Instrument::Nasdaq, autumn, &[(100.0, 110.0, 90.0, 105.0)]);
    let bar = find_signal_bar(&candles, Instrument::Nasdaq, autumn, &config);
    assert!(bar.is_some());
    // 09:45 EST = 14:45 UTC
    assert_eq!(
        bar.unwrap().candle.timestamp.format("%H:%M").to_string(),
        "14:45"
    );
}

#[test]
fn test_signal_bar_dow_winter_and_summer() {
    let config = default_config();

    let winter = date(2024, 2, 12);
    let candles = make_day_candles(
        Instrument::Dow,
        winter,
        &[(37000.0, 37100.0, 36900.0, 37050.0)],
    );
    let bar = find_signal_bar(&candles, Instrument::Dow, winter, &config).unwrap();
    assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "14:45");
    assert_eq!(bar.buy_level, dec!(37102));
    assert_eq!(bar.sell_level, dec!(36898));

    let summer = date(2024, 8, 5);
    let candles = make_day_candles(
        Instrument::Dow,
        summer,
        &[(37000.0, 37100.0, 36900.0, 37050.0)],
    );
    let bar = find_signal_bar(&candles, Instrument::Dow, summer, &config).unwrap();
    assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "13:45");
}

#[test]
fn test_signal_bar_missing_data_returns_none() {
    let d = date(2024, 1, 15);
    let config = default_config();

    // Empty candles
    let result = find_signal_bar(&[], Instrument::Dax, d, &config);
    assert!(result.is_none());

    // Candles at wrong time
    let wrong_candle = make_candle(
        Instrument::Dax,
        "2024-01-15 10:00",
        "100",
        "110",
        "90",
        "105",
    );
    let result = find_signal_bar(&[wrong_candle], Instrument::Dax, d, &config);
    assert!(result.is_none());
}

// ============================================================================
// Order generation edge cases
// ============================================================================

#[test]
fn test_orders_all_sl_modes_all_instruments() {
    let d = date(2024, 1, 15);
    let modes = [
        StopLossMode::SignalBarExtreme,
        StopLossMode::FixedPoints,
        StopLossMode::Midpoint,
    ];

    for instrument in Instrument::ALL {
        for mode in &modes {
            let config = StrategyConfig {
                instrument,
                sl_mode: *mode,
                sl_fixed_points: dec!(40),
                sl_midpoint_offset: dec!(5),
                ..default_config()
            };
            let bar = make_signal_bar(instrument, d, 100.0, 110.0, 90.0, 105.0, &config);
            let orders = generate_orders(&bar, &config);
            assert_eq!(
                orders.len(),
                2,
                "expected 2 orders for {instrument} with {mode}"
            );

            // Buy SL must be below trigger
            assert!(
                orders[0].stop_loss < orders[0].trigger_price,
                "buy SL ({}) >= trigger ({}) for {instrument} {mode}",
                orders[0].stop_loss,
                orders[0].trigger_price
            );
            // Sell SL must be above trigger
            assert!(
                orders[1].stop_loss > orders[1].trigger_price,
                "sell SL ({}) <= trigger ({}) for {instrument} {mode}",
                orders[1].stop_loss,
                orders[1].trigger_price
            );
        }
    }
}

#[test]
fn test_allow_both_sides_false_generates_one_order() {
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        allow_both_sides: false,
        ..default_config()
    };
    let bar = make_signal_bar(
        Instrument::Dax,
        d,
        16000.0,
        16050.0,
        15980.0,
        16030.0,
        &config,
    );
    let orders = generate_orders(&bar, &config);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].direction, Direction::Long);
}

// ============================================================================
// Position update edge cases
// ============================================================================

#[test]
fn test_sl_priority_over_trailing_stop_same_candle() {
    // When both SL and trailing stop could fire on the same candle,
    // SL is checked first (step 1 before step 3)
    use crate::test_helpers::utc;

    let mut position = Position {
        direction: Direction::Long,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(15960),
        size: dec!(1),
        best_price: dec!(16050),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    let config = StrategyConfig {
        exit_mode: ExitMode::TrailingStop,
        trailing_stop_distance: dec!(30),
        trailing_stop_activation: dec!(0),
        ..default_config()
    };

    // Trail level = 16050 - 30 = 16020
    // Candle: low = 15950, touches SL(15960) AND trail(16020)
    // SL should win
    let candle = make_candle(
        Instrument::Dax,
        "2024-01-15 09:00",
        "16010",
        "16020",
        "15950",
        "15970",
    );
    let exit = position.update(&candle, &config).unwrap();
    assert_eq!(exit.exit_reason, PositionStatus::StopLoss);
    assert_eq!(exit.exit_price, dec!(15960));
}

#[test]
fn test_sl_priority_over_eod_same_candle() {
    use crate::test_helpers::utc;

    let mut position = Position {
        direction: Direction::Long,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(15960),
        size: dec!(1),
        best_price: dec!(16000),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    let config = default_config(); // EndOfDay at 17:30 CET

    // EOD candle (16:30 UTC = 17:30 CET) that also hits SL
    let candle = make_candle(
        Instrument::Dax,
        "2024-01-15 16:30",
        "15980",
        "15990",
        "15950",
        "15970",
    );
    let exit = position.update(&candle, &config).unwrap();
    // SL checked first (step 1), before time exit (step 6)
    assert_eq!(exit.exit_reason, PositionStatus::StopLoss);
}

#[test]
fn test_position_update_adds_then_continues() {
    use crate::test_helpers::utc;

    let mut position = Position {
        direction: Direction::Long,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(15960),
        size: dec!(1),
        best_price: dec!(16000),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    let config = StrategyConfig {
        exit_mode: ExitMode::EndOfDay,
        add_to_winners_enabled: true,
        add_every_points: dec!(50),
        max_additions: 3,
        add_size_multiplier: dec!(1),
        move_sl_on_add: true,
        add_sl_offset: dec!(0),
        position_size: dec!(1),
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    // Candle triggers first add (high=16060 >= 16050) but no exit
    let candle = make_candle(
        Instrument::Dax,
        "2024-01-15 09:00",
        "16040",
        "16060",
        "16030",
        "16055",
    );
    let exit = position.update(&candle, &config);
    assert!(exit.is_none());
    assert_eq!(position.adds.len(), 1);
    assert_eq!(position.adds[0].price, dec!(16050));
    // SL tightened to entry price (offset=0)
    assert_eq!(position.stop_loss, dec!(16000));
}

// ============================================================================
// Multi-day known-answer: 3-day trading scenario
// ============================================================================

#[test]
fn test_known_answer_three_day_scenario() {
    let config = StrategyConfig {
        sl_mode: StopLossMode::FixedPoints,
        sl_fixed_points: dec!(40),
        exit_mode: ExitMode::EndOfDay,
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    let mut all_trades = Vec::new();

    // Day 1: Jan 15 - Long trade wins
    {
        let d = date(2024, 1, 15);
        let candles = make_day_candles(
            Instrument::Dax,
            d,
            &[
                (16000.0, 16050.0, 15980.0, 16030.0), // signal bar
                (16040.0, 16060.0, 16020.0, 16055.0), // buy fills
            ],
        );
        let bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&bar, &config);
        let fill = check_fill(&orders[0], &candles[1], dec!(0)).unwrap();

        let mut pos = Position {
            direction: fill.direction,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: dec!(1),
            best_price: fill.fill_price,
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        let eod = make_candle(
            Instrument::Dax,
            "2024-01-15 16:30",
            "16080",
            "16090",
            "16070",
            "16085",
        );
        let exit = pos.update(&eod, &config).unwrap();
        let trade = Position {
            direction: fill.direction,
            entry_price: fill.fill_price,
            entry_time: fill.fill_time,
            stop_loss: orders[0].stop_loss,
            size: dec!(1),
            best_price: dec!(16090),
            adds: Vec::new(),
            status: PositionStatus::Open,
        }
        .close(exit, &config);
        assert_eq!(trade.pnl_points, dec!(33)); // 16085 - 16052
        all_trades.push(trade);
    }

    // Day 2: Jan 16 - Short trade, stop loss
    {
        let d = date(2024, 1, 16);
        let candles = make_day_candles(
            Instrument::Dax,
            d,
            &[
                (16100.0, 16150.0, 16080.0, 16120.0), // signal bar
                (16090.0, 16100.0, 16070.0, 16075.0), // sell fills
                (16080.0, 16130.0, 16070.0, 16125.0), // SL hit
            ],
        );
        let bar = find_signal_bar(&candles, Instrument::Dax, d, &config).unwrap();
        let orders = generate_orders(&bar, &config);
        let sell_order = &orders[1];
        // sell trigger = 16078, SL = 16078 + 40 = 16118
        assert_eq!(sell_order.trigger_price, dec!(16078));
        assert_eq!(sell_order.stop_loss, dec!(16118));

        let fill = check_fill(sell_order, &candles[1], dec!(0)).unwrap();
        assert_eq!(fill.fill_price, dec!(16078));

        let mut pos = Position {
            direction: Direction::Short,
            entry_price: dec!(16078),
            entry_time: fill.fill_time,
            stop_loss: dec!(16118),
            size: dec!(1),
            best_price: dec!(16078),
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        // Bar 3 high = 16130 >= SL 16118
        let exit = pos.update(&candles[2], &config).unwrap();
        assert_eq!(exit.exit_reason, PositionStatus::StopLoss);

        let trade = Position {
            direction: Direction::Short,
            entry_price: dec!(16078),
            entry_time: fill.fill_time,
            stop_loss: dec!(16118),
            size: dec!(1),
            best_price: dec!(16078),
            adds: Vec::new(),
            status: PositionStatus::Open,
        }
        .close(exit, &config);
        assert_eq!(trade.pnl_points, dec!(-40)); // (16118-16078)*-1
        all_trades.push(trade);
    }

    // Day 3: Jan 17 - No signal bar (holiday/missing data)
    {
        let d = date(2024, 1, 17);
        let candles: Vec<crate::models::Candle> = vec![];
        let bar = find_signal_bar(&candles, Instrument::Dax, d, &config);
        assert!(bar.is_none());
    }

    // Summary
    assert_eq!(all_trades.len(), 2);
    let total_pnl: rust_decimal::Decimal = all_trades.iter().map(|t| t.pnl_points).sum();
    assert_eq!(total_pnl, dec!(-7)); // 33 + (-40)
}

// ============================================================================
// Fill simulation edge cases
// ============================================================================

#[test]
fn test_fill_with_both_sides_sell_closer_to_open() {
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        slippage_points: dec!(0),
        ..default_config()
    };

    let bar = make_signal_bar(
        Instrument::Dax,
        d,
        16000.0,
        16050.0,
        15980.0,
        16030.0,
        &config,
    );
    let orders = generate_orders(&bar, &config);

    // Open at 15985: sell trigger at 15978 is closer (dist=7) vs buy trigger at 16052 (dist=67)
    let candle = make_candle(
        Instrument::Dax,
        "2024-01-15 08:30",
        "15985",
        "16060",
        "15970",
        "16000",
    );
    let fills = determine_fill_order(&orders[0], &orders[1], &candle, &config);
    assert_eq!(fills.len(), 2);
    assert_eq!(fills[0].direction, Direction::Short); // sell closer to open
    assert_eq!(fills[1].direction, Direction::Long);
}

#[test]
fn test_order_not_filled_when_never_reaches_trigger() {
    let d = date(2024, 1, 15);
    let config = default_config();

    let bar = make_signal_bar(
        Instrument::Dax,
        d,
        16000.0,
        16050.0,
        15980.0,
        16030.0,
        &config,
    );
    let orders = generate_orders(&bar, &config);

    // Range-bound candle that doesn't touch either trigger
    let candle = make_candle(
        Instrument::Dax,
        "2024-01-15 08:30",
        "16010",
        "16040",
        "15990",
        "16020",
    );

    let buy_fill = check_fill(&orders[0], &candle, config.slippage_points);
    assert!(buy_fill.is_none());
    let sell_fill = check_fill(&orders[1], &candle, config.slippage_points);
    assert!(sell_fill.is_none());
}

// ============================================================================
// Take profit known-answer
// ============================================================================

#[test]
fn test_known_answer_take_profit_long() {
    use crate::test_helpers::utc;

    let config = StrategyConfig {
        exit_mode: ExitMode::FixedTakeProfit,
        fixed_tp_points: dec!(80),
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    let mut position = Position {
        direction: Direction::Long,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(15960),
        size: dec!(1),
        best_price: dec!(16000),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    // TP level = 16000 + 80 = 16080
    // Candle that just misses TP
    let miss = make_candle(
        Instrument::Dax,
        "2024-01-15 09:00",
        "16040",
        "16070",
        "16030",
        "16060",
    );
    assert!(position.update(&miss, &config).is_none());
    assert_eq!(position.best_price, dec!(16070));

    // Candle that hits TP
    let hit = make_candle(
        Instrument::Dax,
        "2024-01-15 09:15",
        "16060",
        "16090",
        "16050",
        "16085",
    );
    let exit = position.update(&hit, &config).unwrap();
    assert_eq!(exit.exit_reason, PositionStatus::TakeProfit);
    assert_eq!(exit.exit_price, dec!(16080)); // exact TP level

    let trade = Position {
        direction: Direction::Long,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(15960),
        size: dec!(1),
        best_price: dec!(16090),
        adds: Vec::new(),
        status: PositionStatus::Open,
    }
    .close(exit, &config);
    assert_eq!(trade.pnl_points, dec!(80));
    assert_eq!(trade.pnl_with_adds, dec!(80));
}

#[test]
fn test_known_answer_take_profit_short() {
    use crate::test_helpers::utc;

    let config = StrategyConfig {
        exit_mode: ExitMode::FixedTakeProfit,
        fixed_tp_points: dec!(60),
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    let mut position = Position {
        direction: Direction::Short,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(16040),
        size: dec!(1),
        best_price: dec!(16000),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    // TP level = 16000 - 60 = 15940
    let hit = make_candle(
        Instrument::Dax,
        "2024-01-15 09:00",
        "15960",
        "15970",
        "15930",
        "15945",
    );
    let exit = position.update(&hit, &config).unwrap();
    assert_eq!(exit.exit_reason, PositionStatus::TakeProfit);
    assert_eq!(exit.exit_price, dec!(15940));

    let trade = Position {
        direction: Direction::Short,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(16040),
        size: dec!(1),
        best_price: dec!(15930),
        adds: Vec::new(),
        status: PositionStatus::Open,
    }
    .close(exit, &config);
    assert_eq!(trade.pnl_points, dec!(60));
}

// ============================================================================
// Commission and slippage integration
// ============================================================================

#[test]
fn test_commission_and_slippage_in_full_flow() {
    use crate::test_helpers::utc;

    let config = StrategyConfig {
        exit_mode: ExitMode::FixedTakeProfit,
        fixed_tp_points: dec!(100),
        slippage_points: dec!(1),
        commission_per_trade: dec!(5),
        ..default_config()
    };

    let position = Position {
        direction: Direction::Long,
        entry_price: dec!(16001), // includes slippage already in fill
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(15960),
        size: dec!(1),
        best_price: dec!(16110),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    let exit = crate::strategy::position::ExitResult {
        exit_price: dec!(16101), // TP at entry+100
        exit_time: utc(2024, 1, 15, 10, 0),
        exit_reason: PositionStatus::TakeProfit,
    };

    let trade = position.close(exit, &config);
    assert_eq!(trade.pnl_points, dec!(100)); // raw PnL
    // 1 fill: commission=5, slippage=1*1*2=2 -> total deduction=7
    assert_eq!(trade.pnl_with_adds, dec!(93));
}

// ============================================================================
// FTSE full flow
// ============================================================================

#[test]
fn test_full_flow_ftse_summer_trailing_stop() {
    // FTSE summer: signal bar at 07:15 UTC (08:15 BST)
    let d = date(2024, 7, 15);
    let config = StrategyConfig {
        instrument: Instrument::Ftse,
        sl_mode: StopLossMode::FixedPoints,
        sl_fixed_points: dec!(30),
        exit_mode: ExitMode::TrailingStop,
        trailing_stop_distance: dec!(20),
        trailing_stop_activation: dec!(10),
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    let candles = make_day_candles(
        Instrument::Ftse,
        d,
        &[
            (7500.0, 7520.0, 7480.0, 7510.0), // signal bar at 07:15 UTC
            (7515.0, 7530.0, 7510.0, 7525.0), // buy fills at 7522
            (7525.0, 7550.0, 7520.0, 7545.0), // price rises
            (7540.0, 7545.0, 7520.0, 7525.0), // trailing stop hit
        ],
    );

    let bar = find_signal_bar(&candles, Instrument::Ftse, d, &config).unwrap();
    assert_eq!(bar.candle.timestamp.format("%H:%M").to_string(), "07:15");
    assert_eq!(bar.buy_level, dec!(7522));

    let orders = generate_orders(&bar, &config);
    let buy_order = &orders[0];
    assert_eq!(buy_order.trigger_price, dec!(7522));
    assert_eq!(buy_order.stop_loss, dec!(7492)); // 7522 - 30

    // Fill on bar 2
    let fill = check_fill(buy_order, &candles[1], dec!(0)).unwrap();
    assert_eq!(fill.fill_price, dec!(7522));

    let mut pos = Position {
        direction: Direction::Long,
        entry_price: dec!(7522),
        entry_time: fill.fill_time,
        stop_loss: dec!(7492),
        size: dec!(1),
        best_price: dec!(7522),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    // Bar 3: high=7550, best_price updates to 7550
    // Unrealized = 7550 - 7522 = 28 >= activation(10), trail = 7550 - 20 = 7530
    // Low=7520 <= 7530? Yes, but SL at 7492 is not hit (low=7520 > 7492)
    // Actually let's trace: step 1 SL check: low=7520 > 7492 -> no SL
    // step 2: best_price = max(7522, 7550) = 7550
    // step 3: unrealized = 7550-7522=28 >= 10. trail = 7550-20=7530. low=7520 <= 7530 -> hit!
    let exit = pos.update(&candles[2], &config);
    assert!(exit.is_some());
    let exit = exit.unwrap();
    assert_eq!(exit.exit_reason, PositionStatus::TrailingStop);
    assert_eq!(exit.exit_price, dec!(7530));
}

// ============================================================================
// Nasdaq full flow
// ============================================================================

#[test]
fn test_full_flow_nasdaq_winter_no_fill() {
    // Signal bar detected but price never reaches trigger levels
    let d = date(2024, 1, 15);
    let config = StrategyConfig {
        instrument: Instrument::Nasdaq,
        slippage_points: dec!(0),
        ..default_config()
    };

    let candles = make_day_candles(
        Instrument::Nasdaq,
        d,
        &[
            (15000.0, 15050.0, 14950.0, 15020.0), // signal bar at 14:45 UTC
            (15010.0, 15040.0, 14960.0, 15030.0), // range-bound, no fills
            (15020.0, 15045.0, 14955.0, 15035.0), // still range-bound
        ],
    );

    let bar = find_signal_bar(&candles, Instrument::Nasdaq, d, &config).unwrap();
    assert_eq!(bar.buy_level, dec!(15052));
    assert_eq!(bar.sell_level, dec!(14948));

    let orders = generate_orders(&bar, &config);

    // Check each subsequent candle: no fills
    for candle in &candles[1..] {
        let buy_fill = check_fill(&orders[0], candle, dec!(0));
        let sell_fill = check_fill(&orders[1], candle, dec!(0));
        assert!(buy_fill.is_none(), "unexpected buy fill");
        assert!(sell_fill.is_none(), "unexpected sell fill");
    }
}

// ============================================================================
// Max additions enforced in integration
// ============================================================================

#[test]
fn test_max_additions_enforced_in_position_update() {
    use crate::test_helpers::utc;

    let config = StrategyConfig {
        exit_mode: ExitMode::EndOfDay,
        add_to_winners_enabled: true,
        add_every_points: dec!(20),
        max_additions: 2,
        add_size_multiplier: dec!(1),
        move_sl_on_add: true,
        add_sl_offset: dec!(0),
        position_size: dec!(1),
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    let mut pos = Position {
        direction: Direction::Long,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(15960),
        size: dec!(1),
        best_price: dec!(16000),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    // Add 1 at 16020
    let c1 = make_candle(
        Instrument::Dax,
        "2024-01-15 08:45",
        "16010",
        "16025",
        "16005",
        "16020",
    );
    assert!(pos.update(&c1, &config).is_none());
    assert_eq!(pos.adds.len(), 1);

    // Add 2 at 16040
    let c2 = make_candle(
        Instrument::Dax,
        "2024-01-15 09:00",
        "16030",
        "16045",
        "16025",
        "16040",
    );
    assert!(pos.update(&c2, &config).is_none());
    assert_eq!(pos.adds.len(), 2);

    // Would-be add 3 at 16060: should NOT trigger (max=2)
    let c3 = make_candle(
        Instrument::Dax,
        "2024-01-15 09:15",
        "16050",
        "16065",
        "16045",
        "16060",
    );
    assert!(pos.update(&c3, &config).is_none());
    assert_eq!(pos.adds.len(), 2); // still 2, no 3rd add
}

// ============================================================================
// ExitMode::None -- position only exits via SL
// ============================================================================

#[test]
fn test_exit_mode_none_only_sl_closes() {
    use crate::test_helpers::utc;

    let config = StrategyConfig {
        exit_mode: ExitMode::None,
        slippage_points: dec!(0),
        commission_per_trade: dec!(0),
        ..default_config()
    };

    let mut pos = Position {
        direction: Direction::Long,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(15960),
        size: dec!(1),
        best_price: dec!(16000),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    // EOD candle -- should NOT close since exit_mode=None
    let eod = make_candle(
        Instrument::Dax,
        "2024-01-15 16:30",
        "16050",
        "16060",
        "16040",
        "16055",
    );
    assert!(pos.update(&eod, &config).is_none());

    // SL candle
    let sl = make_candle(
        Instrument::Dax,
        "2024-01-15 17:00",
        "15980",
        "15990",
        "15950",
        "15960",
    );
    let exit = pos.update(&sl, &config).unwrap();
    assert_eq!(exit.exit_reason, PositionStatus::StopLoss);
}

// ============================================================================
// Verify best_price does not decrease for longs / increase for shorts
// ============================================================================

#[test]
fn test_best_price_monotonic_long() {
    use crate::test_helpers::utc;

    let config = StrategyConfig {
        exit_mode: ExitMode::None,
        ..default_config()
    };

    let mut pos = Position {
        direction: Direction::Long,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(15900),
        size: dec!(1),
        best_price: dec!(16000),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    let c1 = make_candle(
        Instrument::Dax,
        "2024-01-15 08:45",
        "16010",
        "16050",
        "15910",
        "16040",
    );
    pos.update(&c1, &config);
    assert_eq!(pos.best_price, dec!(16050));

    // Lower high: best_price stays at 16050
    let c2 = make_candle(
        Instrument::Dax,
        "2024-01-15 09:00",
        "16020",
        "16030",
        "15910",
        "16025",
    );
    pos.update(&c2, &config);
    assert_eq!(pos.best_price, dec!(16050));

    // New high: best_price updates
    let c3 = make_candle(
        Instrument::Dax,
        "2024-01-15 09:15",
        "16040",
        "16070",
        "15910",
        "16060",
    );
    pos.update(&c3, &config);
    assert_eq!(pos.best_price, dec!(16070));
}

#[test]
fn test_best_price_monotonic_short() {
    use crate::test_helpers::utc;

    let config = StrategyConfig {
        exit_mode: ExitMode::None,
        ..default_config()
    };

    let mut pos = Position {
        direction: Direction::Short,
        entry_price: dec!(16000),
        entry_time: utc(2024, 1, 15, 8, 30),
        stop_loss: dec!(16100),
        size: dec!(1),
        best_price: dec!(16000),
        adds: Vec::new(),
        status: PositionStatus::Open,
    };

    let c1 = make_candle(
        Instrument::Dax,
        "2024-01-15 08:45",
        "15980",
        "16090",
        "15950",
        "15960",
    );
    pos.update(&c1, &config);
    assert_eq!(pos.best_price, dec!(15950));

    // Higher low: best_price stays at 15950
    let c2 = make_candle(
        Instrument::Dax,
        "2024-01-15 09:00",
        "15960",
        "16090",
        "15960",
        "15970",
    );
    pos.update(&c2, &config);
    assert_eq!(pos.best_price, dec!(15950));

    // New low: best_price updates
    let c3 = make_candle(
        Instrument::Dax,
        "2024-01-15 09:15",
        "15950",
        "16090",
        "15930",
        "15940",
    );
    pos.update(&c3, &config);
    assert_eq!(pos.best_price, dec!(15930));
}
