//! Core backtest loop: day-by-day iteration over candles applying the
//! School Run Strategy.
//!
//! The entry point is [`run_backtest`], which takes a slice of candles,
//! an instrument, and a strategy config, and returns a [`BacktestResult`]
//! containing all trades, an equity curve, and daily PnL.

use std::collections::BTreeMap;

use crate::models::{Candle, Instrument};
use crate::strategy::config::StrategyConfig;
use crate::strategy::fill::{FillResult, check_fill, determine_fill_order};
use crate::strategy::order::PendingOrder;
use crate::strategy::position::Position;
use crate::strategy::signal::find_signal_bar;
use crate::strategy::trade::Trade;
use crate::strategy::types::{Direction, PositionStatus};
use chrono::{NaiveDate, TimeZone};

use super::result::BacktestResult;

/// Run a complete backtest over the given candles.
///
/// This is a pure, deterministic function: the same candles and config
/// always produce the same result. No I/O, no randomness.
///
/// # Algorithm
///
/// 1. Group candles by trading date (using the instrument's exchange timezone)
/// 2. For each trading day within `config.date_from..=config.date_to`:
///    a. Skip excluded dates
///    b. Find the signal bar via `find_signal_bar()`
///    c. Generate pending orders via `generate_orders()`
///    d. Set order expiry if `signal_expiry_time` is configured
///    e. Iterate subsequent candles in the session:
///       - Cancel expired unfilled orders
///       - Check pending order fills (handling both-sides-triggered)
///       - Update active positions via `position.update()`
///       - Record completed trades
///         f. Force-close any positions still open at end of day
/// 3. Return a `BacktestResult` with trades, equity curve, and daily PnL
///
/// # Arguments
///
/// * `candles` - All candles for the instrument, sorted by timestamp
/// * `instrument` - The trading instrument
/// * `config` - Strategy configuration
#[must_use]
pub fn run_backtest(
    candles: &[Candle],
    instrument: Instrument,
    config: &StrategyConfig,
) -> BacktestResult {
    let days = group_candles_by_date(candles, instrument);
    let mut trades: Vec<Trade> = Vec::new();

    for date in iter_trading_dates(config.date_from, config.date_to) {
        if config.exclude_dates.contains(&date) {
            continue;
        }

        let day_candles = match days.get(&date) {
            Some(c) => c.as_slice(),
            None => continue,
        };

        let mut day_trades = process_day(day_candles, instrument, date, config);
        trades.append(&mut day_trades);
    }

    BacktestResult::from_trades(instrument, config.clone(), trades)
}

/// Group candles by their trading date in the instrument's exchange timezone.
///
/// Returns a `BTreeMap` so dates are iterated in order.
fn group_candles_by_date(
    candles: &[Candle],
    instrument: Instrument,
) -> BTreeMap<NaiveDate, Vec<&Candle>> {
    let tz = instrument.exchange_timezone();
    let mut map: BTreeMap<NaiveDate, Vec<&Candle>> = BTreeMap::new();

    for candle in candles {
        let local = candle.timestamp.with_timezone(&tz);
        let date = local.date_naive();
        map.entry(date).or_default().push(candle);
    }

    map
}

/// Iterate over each calendar date from `from` to `to` inclusive.
fn iter_trading_dates(from: NaiveDate, to: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    let mut current = from;
    std::iter::from_fn(move || {
        if current > to {
            return None;
        }
        let d = current;
        current = current.succ_opt().unwrap_or(current);
        Some(d)
    })
}

/// Process a single trading day: find signal, generate orders, simulate fills,
/// track positions, and return completed trades.
fn process_day(
    candles: &[&Candle],
    instrument: Instrument,
    date: NaiveDate,
    config: &StrategyConfig,
) -> Vec<Trade> {
    let mut trades: Vec<Trade> = Vec::new();

    // We need owned candles for find_signal_bar (it expects &[Candle])
    let owned_candles: Vec<Candle> = candles.iter().map(|c| (*c).clone()).collect();

    // Step 1: Find signal bar
    let signal_bar = match find_signal_bar(&owned_candles, instrument, date, config) {
        Some(sb) => sb,
        None => return trades, // No signal bar -> skip day
    };

    // Step 2: Generate pending orders
    let mut pending_orders = crate::strategy::order::generate_orders(&signal_bar, config);

    // Step 3: Set expiry on orders if configured
    if let Some(expiry_local) = config.signal_expiry_time {
        let tz = instrument.exchange_timezone();
        let naive_dt = date.and_time(expiry_local);
        if let Some(expiry_utc) = local_to_utc(&tz, &naive_dt) {
            for order in &mut pending_orders {
                order.expires_at = Some(expiry_utc);
            }
        }
    }

    // Step 4: Find candles after the signal bar
    let signal_bar_time = signal_bar.candle.timestamp;
    let post_signal_candles: Vec<&&Candle> = candles
        .iter()
        .filter(|c| c.timestamp > signal_bar_time)
        .collect();

    // Track active positions for this day
    let mut active_positions: Vec<Position> = Vec::new();

    // Step 5: Iterate subsequent candles
    for candle in &post_signal_candles {
        // 5a: Cancel expired unfilled orders
        pending_orders.retain(|order| {
            if let Some(expiry) = order.expires_at {
                candle.timestamp < expiry
            } else {
                true
            }
        });

        // 5b: Check for fills on pending orders
        let fills = get_fills(&pending_orders, candle, config);

        for fill in fills {
            // Remove the filled order from pending
            pending_orders.retain(|o| o.direction != fill.direction);

            // If allow_both_sides is false and we just filled, cancel opposite
            if !config.allow_both_sides {
                pending_orders.clear();
            }

            // Create position from fill
            let position = Position {
                direction: fill.direction,
                entry_price: fill.fill_price,
                entry_time: fill.fill_time,
                stop_loss: fill.order.stop_loss,
                size: fill.order.size,
                best_price: fill.fill_price,
                adds: Vec::new(),
                status: PositionStatus::Open,
            };
            active_positions.push(position);
        }

        // 5c: Update active positions
        let mut closed_indices: Vec<usize> = Vec::new();
        for (i, position) in active_positions.iter_mut().enumerate() {
            if let Some(exit) = position.update(candle, config) {
                let closed_pos = position.clone();
                let trade = closed_pos.close(exit, config);
                trades.push(trade);
                closed_indices.push(i);
            }
        }

        // Remove closed positions (in reverse to preserve indices)
        for i in closed_indices.into_iter().rev() {
            active_positions.remove(i);
        }
    }

    // Step 6: Force-close any positions still open at EOD
    for position in active_positions {
        let eod_trade = force_close_eod(position, &post_signal_candles, config);
        trades.push(eod_trade);
    }

    trades
}

/// Get fill results for pending orders against a candle.
///
/// When both buy and sell orders are pending, uses `determine_fill_order`
/// to decide which triggers first. Otherwise uses `check_fill` directly.
fn get_fills(
    pending_orders: &[PendingOrder],
    candle: &Candle,
    config: &StrategyConfig,
) -> Vec<FillResult> {
    let buy = pending_orders
        .iter()
        .find(|o| o.direction == Direction::Long);
    let sell = pending_orders
        .iter()
        .find(|o| o.direction == Direction::Short);

    match (buy, sell) {
        (Some(b), Some(s)) => determine_fill_order(b, s, candle, config),
        (Some(b), None) => check_fill(b, candle, config.slippage_points)
            .into_iter()
            .collect(),
        (None, Some(s)) => check_fill(s, candle, config.slippage_points)
            .into_iter()
            .collect(),
        (None, None) => Vec::new(),
    }
}

/// Force-close a position at end of day using the last candle's close price.
fn force_close_eod(
    mut position: Position,
    post_signal_candles: &[&&Candle],
    config: &StrategyConfig,
) -> Trade {
    let last_candle = post_signal_candles
        .last()
        .expect("force_close_eod called with no post-signal candles");

    let exit = crate::strategy::position::ExitResult {
        exit_price: last_candle.close,
        exit_time: last_candle.timestamp,
        exit_reason: PositionStatus::EndOfDay,
    };
    position.status = PositionStatus::EndOfDay;
    position.close(exit, config)
}

/// Convert a local NaiveDateTime to UTC using the given timezone.
///
/// Returns `None` if the local time is non-existent (spring-forward gap).
fn local_to_utc(
    tz: &chrono_tz::Tz,
    naive_dt: &chrono::NaiveDateTime,
) -> Option<chrono::DateTime<chrono::Utc>> {
    match tz.from_local_datetime(naive_dt) {
        chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&chrono::Utc)),
        chrono::LocalResult::Ambiguous(earliest, _) => Some(earliest.with_timezone(&chrono::Utc)),
        chrono::LocalResult::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Instrument;
    use crate::strategy::config::StrategyConfig;
    use crate::strategy::types::{Direction, ExitMode, PositionStatus, StopLossMode};
    use crate::test_helpers::{date, make_candle, make_day_candles, utc};
    use rust_decimal_macros::dec;

    fn default_config() -> StrategyConfig {
        StrategyConfig {
            commission_per_trade: dec!(0),
            slippage_points: dec!(0),
            ..StrategyConfig::default()
        }
    }

    /// Build a realistic day of candles for DAX in winter (CET = UTC+1).
    /// Signal bar is at 08:15 UTC (09:15 CET, 2nd candle).
    /// Post-signal candles from 08:30 to 16:30 (17:30 CET = EOD).
    fn make_dax_day_candles(
        day: NaiveDate,
        signal_ohlc: (f64, f64, f64, f64),
        post_signal: &[(f64, f64, f64, f64)],
    ) -> Vec<Candle> {
        let mut all_bars = vec![signal_ohlc];
        all_bars.extend_from_slice(post_signal);
        make_day_candles(Instrument::Dax, day, &all_bars)
    }

    // -- Basic functionality --

    #[test]
    fn test_empty_candles_returns_empty_result() {
        let config = default_config();
        let result = run_backtest(&[], Instrument::Dax, &config);
        assert_eq!(result.trade_count(), 0);
        assert_eq!(result.final_equity(), config.initial_capital);
    }

    #[test]
    fn test_no_signal_bar_day_skipped() {
        // Candles that don't include the signal bar time
        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 09:00",
            "16000",
            "16050",
            "15980",
            "16030",
        );
        let config = default_config();
        let result = run_backtest(&[candle], Instrument::Dax, &config);
        assert_eq!(result.trade_count(), 0);
    }

    #[test]
    fn test_excluded_date_skipped() {
        let d = date(2024, 1, 15);
        let candles = make_dax_day_candles(
            d,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                (16030.0, 16060.0, 16010.0, 16045.0), // triggers buy at 16052
            ],
        );
        let config = StrategyConfig {
            exclude_dates: vec![d],
            ..default_config()
        };
        let result = run_backtest(&candles, Instrument::Dax, &config);
        assert_eq!(result.trade_count(), 0);
    }

    #[test]
    fn test_date_range_filters_candles() {
        let d = date(2024, 1, 15);
        let candles = make_dax_day_candles(
            d,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[(16030.0, 16060.0, 16010.0, 16045.0)],
        );
        // Config date range doesn't include Jan 15
        let config = StrategyConfig {
            date_from: date(2024, 2, 1),
            date_to: date(2024, 2, 28),
            ..default_config()
        };
        let result = run_backtest(&candles, Instrument::Dax, &config);
        assert_eq!(result.trade_count(), 0);
    }

    #[test]
    fn test_single_long_fill_and_eod_close() {
        // DAX winter: signal bar at 08:15 UTC, EOD at 16:30 UTC (17:30 CET)
        let d = date(2024, 1, 15);
        let candles = make_dax_day_candles(
            d,
            // Signal bar: H=16050, L=15980 -> buy_level=16052, sell_level=15978
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                // 08:30 UTC: price goes up, triggers buy stop at 16052
                (16040.0, 16060.0, 16035.0, 16055.0),
                // 08:45 UTC: price continues up, no exit
                (16055.0, 16080.0, 16045.0, 16070.0),
                // ... fill remaining candles to reach 16:30 UTC (33 candles from 08:30)
                // 09:00
                (16070.0, 16090.0, 16060.0, 16080.0),
                (16080.0, 16090.0, 16060.0, 16075.0),
                (16075.0, 16085.0, 16060.0, 16070.0),
                (16070.0, 16080.0, 16055.0, 16065.0),
                (16065.0, 16075.0, 16050.0, 16060.0),
                (16060.0, 16070.0, 16045.0, 16055.0),
                (16055.0, 16065.0, 16040.0, 16050.0),
                (16050.0, 16060.0, 16035.0, 16045.0),
                // 10:30
                (16045.0, 16055.0, 16030.0, 16040.0),
                (16040.0, 16050.0, 16025.0, 16035.0),
                (16035.0, 16045.0, 16020.0, 16030.0),
                (16030.0, 16040.0, 16015.0, 16025.0),
                (16025.0, 16035.0, 16010.0, 16020.0),
                (16020.0, 16030.0, 16005.0, 16015.0),
                (16015.0, 16025.0, 16000.0, 16010.0),
                (16010.0, 16020.0, 15995.0, 16005.0),
                // 12:30
                (16005.0, 16015.0, 15990.0, 16000.0),
                (16000.0, 16010.0, 15985.0, 15995.0),
                (15995.0, 16005.0, 15980.0, 15990.0),
                (15990.0, 16000.0, 15975.0, 15985.0),
                (15985.0, 15995.0, 15970.0, 15980.0),
                (15980.0, 15990.0, 15965.0, 15975.0),
                (15975.0, 15985.0, 15962.0, 15970.0),
                (15970.0, 15980.0, 15962.0, 15975.0),
                // 14:30
                (15975.0, 15985.0, 15962.0, 15980.0),
                (15980.0, 15990.0, 15965.0, 15985.0),
                (15985.0, 15995.0, 15970.0, 15990.0),
                (15990.0, 16000.0, 15975.0, 15995.0),
                (15995.0, 16005.0, 15980.0, 16000.0),
                (16000.0, 16010.0, 15985.0, 16005.0),
                (16005.0, 16015.0, 15990.0, 16010.0),
            ],
        );

        let config = StrategyConfig {
            exit_mode: ExitMode::EndOfDay,
            // EOD at 17:30 CET = 16:30 UTC
            exit_eod_time: chrono::NaiveTime::from_hms_opt(17, 30, 0).unwrap(),
            ..default_config()
        };
        let result = run_backtest(&candles, Instrument::Dax, &config);

        // Should have at least one trade (the long that was filled)
        assert!(result.trade_count() >= 1);
        let trade = &result.trades[0];
        assert_eq!(trade.direction, Direction::Long);
        assert_eq!(trade.entry_price, dec!(16052)); // buy stop trigger
    }

    #[test]
    fn test_single_short_fill_and_stop_loss() {
        let d = date(2024, 1, 15);
        // Signal bar: H=16050, L=15980 -> sell_level=15978, SL for short=16018 (fixed 40pt)
        let candles = make_dax_day_candles(
            d,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                // 08:30: sell stop triggers (low=15970 <= 15978)
                (15990.0, 16000.0, 15970.0, 15985.0),
                // 08:45: price rallies to hit stop loss at 16018 (high=16020 >= 16018)
                (15985.0, 16020.0, 15980.0, 16015.0),
            ],
        );

        let config = StrategyConfig {
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            allow_both_sides: false, // only short side
            ..default_config()
        };

        // Since allow_both_sides=false, only a long order is generated.
        // We need allow_both_sides=true to get the short order.
        let config = StrategyConfig {
            allow_both_sides: true,
            ..config
        };

        let result = run_backtest(&candles, Instrument::Dax, &config);

        // Find the short trade
        let short_trades: Vec<&Trade> = result
            .trades
            .iter()
            .filter(|t| t.direction == Direction::Short)
            .collect();

        if !short_trades.is_empty() {
            let trade = short_trades[0];
            assert_eq!(trade.direction, Direction::Short);
            assert_eq!(trade.exit_reason, PositionStatus::StopLoss);
        }
    }

    #[test]
    fn test_both_sides_triggered_same_session() {
        let d = date(2024, 1, 15);
        // Signal bar: H=16050, L=15980 -> buy=16052, sell=15978
        let candles = make_dax_day_candles(
            d,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                // Wide candle that triggers both sides
                (16015.0, 16060.0, 15970.0, 16000.0),
                // Next candle for position updates
                (16000.0, 16010.0, 15990.0, 16005.0),
            ],
        );

        let config = StrategyConfig {
            allow_both_sides: true,
            ..default_config()
        };
        let result = run_backtest(&candles, Instrument::Dax, &config);

        // Both sides should have triggered
        let has_long = result.trades.iter().any(|t| t.direction == Direction::Long);
        let has_short = result
            .trades
            .iter()
            .any(|t| t.direction == Direction::Short);
        assert!(has_long, "should have a long trade");
        assert!(has_short, "should have a short trade");
    }

    #[test]
    fn test_signal_expiry_cancels_unfilled_orders() {
        let d = date(2024, 1, 15);
        // Signal bar at 08:15 UTC
        let candles = make_dax_day_candles(
            d,
            // Signal bar: H=16050, L=15980 -> buy=16052
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                // 08:30: no fill (high only 16040)
                (16020.0, 16040.0, 16010.0, 16030.0),
                // 08:45: no fill
                (16025.0, 16035.0, 16015.0, 16020.0),
                // 09:00: no fill
                (16015.0, 16030.0, 16010.0, 16025.0),
                // 09:15: would fill (high=16060) but order should be expired by now
                (16040.0, 16060.0, 16035.0, 16055.0),
            ],
        );

        let config = StrategyConfig {
            // Expire orders at 10:00 CET = 09:00 UTC in winter
            signal_expiry_time: Some(chrono::NaiveTime::from_hms_opt(10, 0, 0).unwrap()),
            ..default_config()
        };
        let result = run_backtest(&candles, Instrument::Dax, &config);

        // The 09:15 UTC candle is at 10:15 CET, after expiry at 10:00 CET
        // So no trades should be generated
        assert_eq!(
            result.trade_count(),
            0,
            "orders should have expired before fill"
        );
    }

    #[test]
    fn test_stop_loss_exit() {
        let d = date(2024, 1, 15);
        // Signal bar H=16050, L=15980 -> buy=16052, SL=16012 (fixed 40pt)
        let candles = make_dax_day_candles(
            d,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                // 08:30: triggers buy at 16052
                (16040.0, 16060.0, 16035.0, 16055.0),
                // 08:45: price drops to hit SL at 16012 (low=16010 <= 16012)
                (16040.0, 16045.0, 16010.0, 16015.0),
            ],
        );

        let config = StrategyConfig {
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            allow_both_sides: false,
            ..default_config()
        };
        // allow_both_sides=false only generates the long order
        let result = run_backtest(&candles, Instrument::Dax, &config);

        assert_eq!(result.trade_count(), 1);
        let trade = &result.trades[0];
        assert_eq!(trade.direction, Direction::Long);
        assert_eq!(trade.exit_reason, PositionStatus::StopLoss);
        assert_eq!(trade.exit_price, dec!(16012));
        assert_eq!(trade.pnl_points, dec!(-40));
    }

    #[test]
    fn test_take_profit_exit() {
        let d = date(2024, 1, 15);
        // buy_level = 16052, TP = entry + 100 = 16152
        let candles = make_dax_day_candles(
            d,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                // 08:30: triggers buy
                (16040.0, 16060.0, 16035.0, 16055.0),
                // 08:45: price rises to TP
                (16100.0, 16160.0, 16090.0, 16150.0),
            ],
        );

        let config = StrategyConfig {
            exit_mode: ExitMode::FixedTakeProfit,
            fixed_tp_points: dec!(100),
            allow_both_sides: false,
            ..default_config()
        };
        let result = run_backtest(&candles, Instrument::Dax, &config);

        assert_eq!(result.trade_count(), 1);
        let trade = &result.trades[0];
        assert_eq!(trade.exit_reason, PositionStatus::TakeProfit);
        assert_eq!(trade.exit_price, dec!(16152));
        assert_eq!(trade.pnl_points, dec!(100));
    }

    #[test]
    fn test_deterministic_same_input_same_output() {
        let d = date(2024, 1, 15);
        let candles = make_dax_day_candles(
            d,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                (16040.0, 16060.0, 16035.0, 16055.0),
                (16055.0, 16080.0, 16045.0, 16070.0),
            ],
        );

        let config = default_config();
        let result1 = run_backtest(&candles, Instrument::Dax, &config);
        let result2 = run_backtest(&candles, Instrument::Dax, &config);

        assert_eq!(result1.trade_count(), result2.trade_count());
        for (t1, t2) in result1.trades.iter().zip(result2.trades.iter()) {
            assert_eq!(t1.entry_price, t2.entry_price);
            assert_eq!(t1.exit_price, t2.exit_price);
            assert_eq!(t1.pnl_points, t2.pnl_points);
            assert_eq!(t1.direction, t2.direction);
        }
    }

    #[test]
    fn test_multi_day_backtest() {
        let d1 = date(2024, 1, 15);
        let d2 = date(2024, 1, 16);

        let mut candles = make_dax_day_candles(
            d1,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[(16040.0, 16060.0, 16035.0, 16055.0)],
        );
        candles.extend(make_dax_day_candles(
            d2,
            (16100.0, 16150.0, 16080.0, 16130.0),
            &[(16130.0, 16160.0, 16120.0, 16145.0)],
        ));

        let config = default_config();
        let result = run_backtest(&candles, Instrument::Dax, &config);

        // Should have trades from both days
        assert!(
            result.trade_count() >= 2,
            "should have trades from multiple days"
        );
    }

    #[test]
    fn test_group_candles_by_date_dax_winter() {
        let d = date(2024, 1, 15);
        let candles = make_day_candles(
            Instrument::Dax,
            d,
            &[
                (16000.0, 16050.0, 15980.0, 16030.0),
                (16030.0, 16060.0, 16010.0, 16045.0),
            ],
        );
        let grouped = group_candles_by_date(&candles, Instrument::Dax);
        assert!(grouped.contains_key(&d));
        assert_eq!(grouped[&d].len(), 2);
    }

    #[test]
    fn test_force_close_eod_produces_trade() {
        let position = Position {
            direction: Direction::Long,
            entry_price: dec!(16052),
            entry_time: utc(2024, 1, 15, 8, 30),
            stop_loss: dec!(16012),
            size: dec!(1),
            best_price: dec!(16080),
            adds: Vec::new(),
            status: PositionStatus::Open,
        };

        let candle = make_candle(
            Instrument::Dax,
            "2024-01-15 16:30",
            "16070",
            "16080",
            "16060",
            "16075",
        );
        let candle_ref = &candle;
        let candle_refs = vec![&candle_ref];

        let config = default_config();
        let trade = force_close_eod(position, &candle_refs, &config);

        assert_eq!(trade.direction, Direction::Long);
        assert_eq!(trade.exit_price, dec!(16075));
        assert_eq!(trade.exit_reason, PositionStatus::EndOfDay);
    }

    #[test]
    fn test_trailing_stop_exit() {
        let d = date(2024, 1, 15);
        let candles = make_dax_day_candles(
            d,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                // 08:30: triggers buy at 16052
                (16040.0, 16060.0, 16035.0, 16055.0),
                // 08:45: price rallies to 16100 (best_price update)
                (16055.0, 16100.0, 16050.0, 16090.0),
                // 09:00: price drops, trailing stop = 16100-30=16070, low=16060 <= 16070
                (16080.0, 16085.0, 16060.0, 16065.0),
            ],
        );

        let config = StrategyConfig {
            exit_mode: ExitMode::TrailingStop,
            trailing_stop_distance: dec!(30),
            trailing_stop_activation: dec!(0),
            allow_both_sides: false,
            ..default_config()
        };
        let result = run_backtest(&candles, Instrument::Dax, &config);

        assert_eq!(result.trade_count(), 1);
        let trade = &result.trades[0];
        assert_eq!(trade.exit_reason, PositionStatus::TrailingStop);
        assert_eq!(trade.exit_price, dec!(16070));
    }

    #[test]
    fn test_no_fill_day_produces_no_trades() {
        let d = date(2024, 1, 15);
        // Signal bar H=16050, L=15980 -> buy=16052, sell=15978
        let candles = make_dax_day_candles(
            d,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                // Post-signal candles never reach trigger levels
                (16010.0, 16040.0, 15990.0, 16020.0),
                (16015.0, 16035.0, 15995.0, 16025.0),
            ],
        );

        let config = default_config();
        let result = run_backtest(&candles, Instrument::Dax, &config);
        assert_eq!(result.trade_count(), 0);
    }

    #[test]
    fn test_config_with_slippage_and_commission() {
        let d = date(2024, 1, 15);
        let candles = make_dax_day_candles(
            d,
            (16000.0, 16050.0, 15980.0, 16030.0),
            &[
                // Triggers buy
                (16040.0, 16060.0, 16035.0, 16055.0),
                // SL hit
                (16040.0, 16045.0, 16005.0, 16010.0),
            ],
        );

        let config = StrategyConfig {
            sl_mode: StopLossMode::FixedPoints,
            sl_fixed_points: dec!(40),
            allow_both_sides: false,
            slippage_points: dec!(1),
            commission_per_trade: dec!(5),
            ..default_config()
        };

        let result = run_backtest(&candles, Instrument::Dax, &config);
        assert_eq!(result.trade_count(), 1);
        let trade = &result.trades[0];
        // Entry = 16052 + 1 (slippage) = 16053
        assert_eq!(trade.entry_price, dec!(16053));
    }

    #[test]
    fn test_iter_trading_dates() {
        let dates: Vec<NaiveDate> =
            iter_trading_dates(date(2024, 1, 1), date(2024, 1, 5)).collect();
        assert_eq!(dates.len(), 5);
        assert_eq!(dates[0], date(2024, 1, 1));
        assert_eq!(dates[4], date(2024, 1, 5));
    }

    #[test]
    fn test_iter_trading_dates_single_day() {
        let dates: Vec<NaiveDate> =
            iter_trading_dates(date(2024, 1, 15), date(2024, 1, 15)).collect();
        assert_eq!(dates.len(), 1);
    }

    #[test]
    fn test_iter_trading_dates_empty_range() {
        let dates: Vec<NaiveDate> =
            iter_trading_dates(date(2024, 2, 1), date(2024, 1, 1)).collect();
        assert_eq!(dates.len(), 0);
    }
}
