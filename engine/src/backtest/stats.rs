//! Backtest statistics computation.
//!
//! [`compute_stats`] takes the completed trades and equity curve from a backtest
//! run and produces a [`BacktestStats`] summary with win rate, profit factor,
//! drawdown, risk-adjusted ratios, and long/short breakdowns.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::strategy::trade::Trade;
use crate::strategy::types::Direction;

use super::result::EquityPoint;

/// Number of trading days per year, used for annualising returns and ratios.
const TRADING_DAYS_PER_YEAR: f64 = 252.0;

/// Custom serde for f64 values that may be infinite or NaN.
///
/// JSON does not support `Infinity` or `NaN` natively. This module
/// serializes such values as strings and deserializes them back.
mod serde_f64_nonfinite {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if value.is_infinite() {
            if *value > 0.0 {
                serializer.serialize_str("Infinity")
            } else {
                serializer.serialize_str("-Infinity")
            }
        } else if value.is_nan() {
            serializer.serialize_str("NaN")
        } else {
            serializer.serialize_f64(*value)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum F64OrString {
            F(f64),
            S(String),
        }

        match F64OrString::deserialize(deserializer)? {
            F64OrString::F(v) => Ok(v),
            F64OrString::S(s) => match s.as_str() {
                "Infinity" | "inf" => Ok(f64::INFINITY),
                "-Infinity" | "-inf" => Ok(f64::NEG_INFINITY),
                "NaN" | "nan" => Ok(f64::NAN),
                other => other.parse::<f64>().map_err(serde::de::Error::custom),
            },
        }
    }
}

/// Comprehensive statistics for a completed backtest.
///
/// Monetary values (PnL, drawdown) use [`Decimal`] for exact arithmetic.
/// Statistical ratios (Sharpe, Sortino, win rate) use [`f64`] where exact
/// precision is not critical (per `CLAUDE.md` convention).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestStats {
    /// Total number of completed trades.
    pub total_trades: u32,
    /// Number of winning trades (PnL > 0).
    pub winning_trades: u32,
    /// Number of losing trades (PnL < 0).
    pub losing_trades: u32,
    /// Fraction of trades that were winners (0.0..=1.0).
    pub win_rate: f64,

    /// Net PnL across all trades.
    pub total_pnl: Decimal,
    /// Average PnL of winning trades.
    pub avg_win: Decimal,
    /// Average PnL of losing trades (negative value).
    pub avg_loss: Decimal,
    /// Largest single winning trade PnL.
    pub largest_win: Decimal,
    /// Largest single losing trade PnL (negative value).
    pub largest_loss: Decimal,

    /// Gross wins / gross losses. Infinite if no losing trades.
    #[serde(with = "serde_f64_nonfinite")]
    pub profit_factor: f64,

    /// Maximum peak-to-trough equity drawdown in absolute terms.
    pub max_drawdown: Decimal,
    /// Maximum peak-to-trough equity drawdown as a percentage of the peak.
    pub max_drawdown_pct: f64,

    /// Annualised Sharpe ratio (daily returns, risk-free rate = 0).
    pub sharpe_ratio: f64,
    /// Annualised Sortino ratio (downside deviation only, risk-free = 0).
    pub sortino_ratio: f64,
    /// Calmar ratio: annualised return / max drawdown.
    pub calmar_ratio: f64,

    /// Longest consecutive winning streak.
    pub max_consecutive_wins: u32,
    /// Longest consecutive losing streak.
    pub max_consecutive_losses: u32,

    /// Average trade duration in minutes.
    pub avg_trade_duration_minutes: f64,

    /// Total number of long trades.
    pub long_trades: u32,
    /// Total number of short trades.
    pub short_trades: u32,
    /// Total PnL from long trades.
    pub long_pnl: Decimal,
    /// Total PnL from short trades.
    pub short_pnl: Decimal,
}

/// Compute comprehensive backtest statistics from trades and the equity curve.
///
/// # Arguments
///
/// * `trades` - All completed trades from the backtest, in chronological order.
/// * `equity_curve` - Equity values at each point in time, produced by the
///   backtest engine.
/// * `initial_capital` - The starting capital, used for drawdown percentage
///   and return calculations.
///
/// # Edge cases
///
/// - **Zero trades**: returns zeroed stats (win_rate 0, ratios 0, etc.).
/// - **One trade**: ratios requiring variance (Sharpe, Sortino) return 0.
/// - **All wins**: profit_factor = `f64::INFINITY`, max_consecutive_losses = 0.
/// - **All losses**: profit_factor = 0, max_consecutive_wins = 0.
/// - **Zero standard deviation**: Sharpe/Sortino return 0 (avoid division by zero).
#[must_use]
pub fn compute_stats(
    trades: &[Trade],
    equity_curve: &[EquityPoint],
    initial_capital: Decimal,
) -> BacktestStats {
    if trades.is_empty() {
        return BacktestStats::empty();
    }

    let (winning, losing) = partition_trades(trades);
    let total_trades = trades.len() as u32;
    let winning_trades = winning.len() as u32;
    let losing_trades = losing.len() as u32;
    let win_rate = winning_trades as f64 / total_trades as f64;

    let total_pnl: Decimal = trades.iter().map(|t| t.pnl_with_adds).sum();
    let gross_wins: Decimal = winning.iter().map(|t| t.pnl_with_adds).sum();
    let gross_losses: Decimal = losing.iter().map(|t| t.pnl_with_adds.abs()).sum();

    let avg_win = if winning.is_empty() {
        Decimal::ZERO
    } else {
        gross_wins / Decimal::from(winning_trades)
    };

    let avg_loss = if losing.is_empty() {
        Decimal::ZERO
    } else {
        let total_loss: Decimal = losing.iter().map(|t| t.pnl_with_adds).sum();
        total_loss / Decimal::from(losing_trades)
    };

    let largest_win = winning
        .iter()
        .map(|t| t.pnl_with_adds)
        .max()
        .unwrap_or(Decimal::ZERO);

    let largest_loss = losing
        .iter()
        .map(|t| t.pnl_with_adds)
        .min()
        .unwrap_or(Decimal::ZERO);

    let profit_factor = if gross_losses.is_zero() {
        if gross_wins.is_zero() { 0.0 } else { f64::MAX }
    } else {
        decimal_to_f64(gross_wins) / decimal_to_f64(gross_losses)
    };

    let (max_drawdown, max_drawdown_pct) = compute_max_drawdown(equity_curve);

    let daily_returns = compute_daily_returns(equity_curve);
    let sharpe_ratio = compute_sharpe(&daily_returns);
    let sortino_ratio = compute_sortino(&daily_returns);
    let calmar_ratio = compute_calmar(equity_curve, initial_capital, max_drawdown);

    let (max_consecutive_wins, max_consecutive_losses) = compute_streaks(trades);
    let avg_trade_duration_minutes = compute_avg_duration(trades);

    let (long_trades, short_trades, long_pnl, short_pnl) = compute_direction_breakdown(trades);

    BacktestStats {
        total_trades,
        winning_trades,
        losing_trades,
        win_rate,
        total_pnl,
        avg_win,
        avg_loss,
        largest_win,
        largest_loss,
        profit_factor,
        max_drawdown,
        max_drawdown_pct,
        sharpe_ratio,
        sortino_ratio,
        calmar_ratio,
        max_consecutive_wins,
        max_consecutive_losses,
        avg_trade_duration_minutes,
        long_trades,
        short_trades,
        long_pnl,
        short_pnl,
    }
}

impl BacktestStats {
    /// Returns a zeroed-out stats struct for backtests with no trades.
    #[must_use]
    fn empty() -> Self {
        Self {
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate: 0.0,
            total_pnl: Decimal::ZERO,
            avg_win: Decimal::ZERO,
            avg_loss: Decimal::ZERO,
            largest_win: Decimal::ZERO,
            largest_loss: Decimal::ZERO,
            profit_factor: 0.0,
            max_drawdown: Decimal::ZERO,
            max_drawdown_pct: 0.0,
            sharpe_ratio: 0.0,
            sortino_ratio: 0.0,
            calmar_ratio: 0.0,
            max_consecutive_wins: 0,
            max_consecutive_losses: 0,
            avg_trade_duration_minutes: 0.0,
            long_trades: 0,
            short_trades: 0,
            long_pnl: Decimal::ZERO,
            short_pnl: Decimal::ZERO,
        }
    }
}

/// Partition trades into winners (PnL > 0) and losers (PnL < 0).
///
/// Break-even trades (PnL == 0) are counted as neither.
fn partition_trades(trades: &[Trade]) -> (Vec<&Trade>, Vec<&Trade>) {
    let mut winners = Vec::new();
    let mut losers = Vec::new();
    for trade in trades {
        if trade.pnl_with_adds > Decimal::ZERO {
            winners.push(trade);
        } else if trade.pnl_with_adds < Decimal::ZERO {
            losers.push(trade);
        }
    }
    (winners, losers)
}

/// Compute peak-to-trough max drawdown from the equity curve.
///
/// Returns `(max_drawdown_absolute, max_drawdown_pct)`.
/// If the equity curve has fewer than 2 points, returns `(0, 0.0)`.
fn compute_max_drawdown(equity_curve: &[EquityPoint]) -> (Decimal, f64) {
    if equity_curve.len() < 2 {
        return (Decimal::ZERO, 0.0);
    }

    let mut peak = equity_curve[0].equity;
    let mut max_dd = Decimal::ZERO;
    let mut max_dd_pct: f64 = 0.0;

    for point in equity_curve {
        if point.equity > peak {
            peak = point.equity;
        }
        let dd = peak - point.equity;
        if dd > max_dd {
            max_dd = dd;
            if !peak.is_zero() {
                max_dd_pct = decimal_to_f64(dd) / decimal_to_f64(peak);
            }
        }
    }

    (max_dd, max_dd_pct)
}

/// Compute simple daily returns from the equity curve.
///
/// Groups equity points by date and takes the last equity value per day.
/// Returns `(equity[i] - equity[i-1]) / equity[i-1]` for consecutive days.
fn compute_daily_returns(equity_curve: &[EquityPoint]) -> Vec<f64> {
    if equity_curve.len() < 2 {
        return Vec::new();
    }

    // Get the last equity point per calendar day.
    let mut daily_equity: Vec<(chrono::NaiveDate, Decimal)> = Vec::new();
    for point in equity_curve {
        let date = point.timestamp.date_naive();
        match daily_equity.last_mut() {
            Some(last) if last.0 == date => {
                last.1 = point.equity;
            }
            _ => {
                daily_equity.push((date, point.equity));
            }
        }
    }

    if daily_equity.len() < 2 {
        return Vec::new();
    }

    daily_equity
        .windows(2)
        .filter_map(|w| {
            let prev = decimal_to_f64(w[0].1);
            let curr = decimal_to_f64(w[1].1);
            if prev == 0.0 {
                None
            } else {
                Some((curr - prev) / prev)
            }
        })
        .collect()
}

/// Annualised Sharpe ratio from daily returns (risk-free rate = 0).
///
/// `Sharpe = mean(daily_returns) / std(daily_returns) * sqrt(252)`
///
/// Returns 0.0 if fewer than 2 returns or zero standard deviation.
fn compute_sharpe(daily_returns: &[f64]) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }

    let mean = daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;
    let variance = daily_returns
        .iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>()
        / (daily_returns.len() - 1) as f64;
    let std_dev = variance.sqrt();

    if std_dev < f64::EPSILON {
        return 0.0;
    }

    (mean / std_dev) * TRADING_DAYS_PER_YEAR.sqrt()
}

/// Annualised Sortino ratio from daily returns (risk-free rate = 0).
///
/// `Sortino = mean(daily_returns) / downside_deviation * sqrt(252)`
///
/// Downside deviation uses only negative returns.
/// Returns 0.0 if fewer than 2 returns or zero downside deviation.
fn compute_sortino(daily_returns: &[f64]) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }

    let mean = daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;

    let downside_sum: f64 = daily_returns
        .iter()
        .filter(|&&r| r < 0.0)
        .map(|r| r.powi(2))
        .sum();
    let downside_variance = downside_sum / (daily_returns.len() - 1) as f64;
    let downside_dev = downside_variance.sqrt();

    if downside_dev < f64::EPSILON {
        return 0.0;
    }

    (mean / downside_dev) * TRADING_DAYS_PER_YEAR.sqrt()
}

/// Calmar ratio: annualised return / max drawdown.
///
/// Returns 0.0 if max drawdown is zero or the equity curve has fewer than 2 points.
fn compute_calmar(
    equity_curve: &[EquityPoint],
    initial_capital: Decimal,
    max_drawdown: Decimal,
) -> f64 {
    if equity_curve.len() < 2 || max_drawdown.is_zero() || initial_capital.is_zero() {
        return 0.0;
    }

    let first_date = equity_curve.first().map(|p| p.timestamp.date_naive());
    let last_date = equity_curve.last().map(|p| p.timestamp.date_naive());

    let (first, last) = match (first_date, last_date) {
        (Some(f), Some(l)) => (f, l),
        _ => return 0.0,
    };

    let days = (last - first).num_days();
    if days <= 0 {
        return 0.0;
    }

    let final_equity = equity_curve
        .last()
        .map(|p| p.equity)
        .unwrap_or(initial_capital);
    let total_return =
        decimal_to_f64(final_equity - initial_capital) / decimal_to_f64(initial_capital);
    let years = days as f64 / 365.25;
    let annualised_return = (1.0 + total_return).powf(1.0 / years) - 1.0;

    annualised_return / decimal_to_f64(max_drawdown / initial_capital)
}

/// Compute maximum consecutive wins and losses.
fn compute_streaks(trades: &[Trade]) -> (u32, u32) {
    let mut max_wins: u32 = 0;
    let mut max_losses: u32 = 0;
    let mut current_wins: u32 = 0;
    let mut current_losses: u32 = 0;

    for trade in trades {
        if trade.pnl_with_adds > Decimal::ZERO {
            current_wins += 1;
            current_losses = 0;
            max_wins = max_wins.max(current_wins);
        } else if trade.pnl_with_adds < Decimal::ZERO {
            current_losses += 1;
            current_wins = 0;
            max_losses = max_losses.max(current_losses);
        } else {
            // Break-even resets both streaks
            current_wins = 0;
            current_losses = 0;
        }
    }

    (max_wins, max_losses)
}

/// Compute average trade duration in minutes.
fn compute_avg_duration(trades: &[Trade]) -> f64 {
    if trades.is_empty() {
        return 0.0;
    }

    let total_minutes: i64 = trades
        .iter()
        .map(|t| (t.exit_time - t.entry_time).num_minutes())
        .sum();

    total_minutes as f64 / trades.len() as f64
}

/// Compute long/short trade breakdown.
///
/// Returns `(long_count, short_count, long_pnl, short_pnl)`.
fn compute_direction_breakdown(trades: &[Trade]) -> (u32, u32, Decimal, Decimal) {
    let mut long_count: u32 = 0;
    let mut short_count: u32 = 0;
    let mut long_pnl = Decimal::ZERO;
    let mut short_pnl = Decimal::ZERO;

    for trade in trades {
        match trade.direction {
            Direction::Long => {
                long_count += 1;
                long_pnl += trade.pnl_with_adds;
            }
            Direction::Short => {
                short_count += 1;
                short_pnl += trade.pnl_with_adds;
            }
        }
    }

    (long_count, short_count, long_pnl, short_pnl)
}

/// Convert a [`Decimal`] to [`f64`] for statistical calculations.
fn decimal_to_f64(d: Decimal) -> f64 {
    use rust_decimal::prelude::ToPrimitive;
    d.to_f64().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Instrument;
    use crate::strategy::types::PositionStatus;
    use crate::test_helpers::utc;
    use chrono::DateTime;
    use rust_decimal_macros::dec;

    fn make_trade(
        direction: Direction,
        entry_price: Decimal,
        exit_price: Decimal,
        entry_time: DateTime<chrono::Utc>,
        exit_time: DateTime<chrono::Utc>,
        pnl: Decimal,
    ) -> Trade {
        Trade {
            instrument: Instrument::Dax,
            direction,
            entry_price,
            exit_price,
            entry_time,
            exit_time,
            stop_loss: dec!(15960),
            exit_reason: if pnl > Decimal::ZERO {
                PositionStatus::TakeProfit
            } else {
                PositionStatus::StopLoss
            },
            pnl_points: pnl,
            pnl_with_adds: pnl,
            adds: Vec::new(),
            size: dec!(1),
        }
    }

    fn make_equity_curve(values: &[(i64, f64)]) -> Vec<EquityPoint> {
        values
            .iter()
            .map(|&(offset_hours, equity)| EquityPoint {
                timestamp: utc(2024, 1, 15, 8, 0) + chrono::Duration::hours(offset_hours),
                equity: Decimal::try_from(equity).unwrap_or(Decimal::ZERO),
            })
            .collect()
    }

    fn make_multi_day_equity(days_and_values: &[(u32, f64)]) -> Vec<EquityPoint> {
        days_and_values
            .iter()
            .map(|&(day, equity)| EquityPoint {
                timestamp: utc(2024, 1, day, 17, 0),
                equity: Decimal::try_from(equity).unwrap_or(Decimal::ZERO),
            })
            .collect()
    }

    // -- Zero trades --

    #[test]
    fn test_empty_trades_returns_zeroed_stats() {
        let stats = compute_stats(&[], &[], dec!(100000));
        assert_eq!(stats.total_trades, 0);
        assert_eq!(stats.win_rate, 0.0);
        assert_eq!(stats.total_pnl, Decimal::ZERO);
        assert_eq!(stats.sharpe_ratio, 0.0);
        assert_eq!(stats.sortino_ratio, 0.0);
        assert_eq!(stats.calmar_ratio, 0.0);
        assert_eq!(stats.max_drawdown, Decimal::ZERO);
        assert_eq!(stats.profit_factor, 0.0);
    }

    // -- Single trade --

    #[test]
    fn test_single_winning_trade() {
        let trades = vec![make_trade(
            Direction::Long,
            dec!(16000),
            dec!(16050),
            utc(2024, 1, 15, 8, 30),
            utc(2024, 1, 15, 10, 0),
            dec!(50),
        )];
        let equity = make_equity_curve(&[(0, 100000.0), (2, 100050.0)]);
        let stats = compute_stats(&trades, &equity, dec!(100000));

        assert_eq!(stats.total_trades, 1);
        assert_eq!(stats.winning_trades, 1);
        assert_eq!(stats.losing_trades, 0);
        assert_eq!(stats.win_rate, 1.0);
        assert_eq!(stats.total_pnl, dec!(50));
        assert_eq!(stats.avg_win, dec!(50));
        assert_eq!(stats.avg_loss, Decimal::ZERO);
        assert_eq!(stats.largest_win, dec!(50));
        assert_eq!(stats.largest_loss, Decimal::ZERO);
        assert_eq!(stats.profit_factor, f64::MAX);
        assert_eq!(stats.max_consecutive_wins, 1);
        assert_eq!(stats.max_consecutive_losses, 0);
        assert_eq!(stats.long_trades, 1);
        assert_eq!(stats.short_trades, 0);
        assert_eq!(stats.long_pnl, dec!(50));
        assert_eq!(stats.short_pnl, Decimal::ZERO);
    }

    #[test]
    fn test_single_losing_trade() {
        let trades = vec![make_trade(
            Direction::Short,
            dec!(16000),
            dec!(16040),
            utc(2024, 1, 15, 8, 30),
            utc(2024, 1, 15, 9, 0),
            dec!(-40),
        )];
        let equity = make_equity_curve(&[(0, 100000.0), (1, 99960.0)]);
        let stats = compute_stats(&trades, &equity, dec!(100000));

        assert_eq!(stats.total_trades, 1);
        assert_eq!(stats.winning_trades, 0);
        assert_eq!(stats.losing_trades, 1);
        assert_eq!(stats.win_rate, 0.0);
        assert_eq!(stats.total_pnl, dec!(-40));
        assert_eq!(stats.avg_win, Decimal::ZERO);
        assert_eq!(stats.avg_loss, dec!(-40));
        assert_eq!(stats.profit_factor, 0.0);
        assert_eq!(stats.max_consecutive_wins, 0);
        assert_eq!(stats.max_consecutive_losses, 1);
        assert_eq!(stats.short_trades, 1);
        assert_eq!(stats.short_pnl, dec!(-40));
    }

    // -- Multiple trades --

    #[test]
    fn test_mixed_trades_win_rate() {
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                utc(2024, 1, 15, 8, 30),
                utc(2024, 1, 15, 10, 0),
                dec!(50),
            ),
            make_trade(
                Direction::Short,
                dec!(16000),
                dec!(16040),
                utc(2024, 1, 16, 8, 30),
                utc(2024, 1, 16, 9, 0),
                dec!(-40),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16030),
                utc(2024, 1, 17, 8, 30),
                utc(2024, 1, 17, 11, 0),
                dec!(30),
            ),
        ];
        let equity = make_multi_day_equity(&[
            (15, 100000.0),
            (16, 100050.0),
            (17, 100010.0),
            (18, 100040.0),
        ]);
        let stats = compute_stats(&trades, &equity, dec!(100000));

        assert_eq!(stats.total_trades, 3);
        assert_eq!(stats.winning_trades, 2);
        assert_eq!(stats.losing_trades, 1);
        assert!((stats.win_rate - 2.0 / 3.0).abs() < 1e-10);
        assert_eq!(stats.total_pnl, dec!(40));
        assert_eq!(stats.avg_win, dec!(40)); // (50 + 30) / 2
        assert_eq!(stats.avg_loss, dec!(-40));
        assert_eq!(stats.largest_win, dec!(50));
        assert_eq!(stats.largest_loss, dec!(-40));
        assert_eq!(stats.long_trades, 2);
        assert_eq!(stats.short_trades, 1);
        assert_eq!(stats.long_pnl, dec!(80));
        assert_eq!(stats.short_pnl, dec!(-40));
    }

    #[test]
    fn test_profit_factor_calculation() {
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16100),
                utc(2024, 1, 15, 8, 30),
                utc(2024, 1, 15, 10, 0),
                dec!(100),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(15960),
                utc(2024, 1, 16, 8, 30),
                utc(2024, 1, 16, 9, 0),
                dec!(-40),
            ),
        ];
        let equity = make_multi_day_equity(&[(15, 100000.0), (16, 100100.0), (17, 100060.0)]);
        let stats = compute_stats(&trades, &equity, dec!(100000));

        // profit_factor = 100 / 40 = 2.5
        assert!((stats.profit_factor - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_all_wins_profit_factor_infinite() {
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                utc(2024, 1, 15, 8, 30),
                utc(2024, 1, 15, 10, 0),
                dec!(50),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16030),
                utc(2024, 1, 16, 8, 30),
                utc(2024, 1, 16, 10, 0),
                dec!(30),
            ),
        ];
        let equity = make_multi_day_equity(&[(15, 100000.0), (16, 100050.0), (17, 100080.0)]);
        let stats = compute_stats(&trades, &equity, dec!(100000));

        assert_eq!(stats.profit_factor, f64::MAX);
        assert_eq!(stats.max_consecutive_wins, 2);
        assert_eq!(stats.max_consecutive_losses, 0);
    }

    #[test]
    fn test_all_losses_profit_factor_zero() {
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(15960),
                utc(2024, 1, 15, 8, 30),
                utc(2024, 1, 15, 9, 0),
                dec!(-40),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(15970),
                utc(2024, 1, 16, 8, 30),
                utc(2024, 1, 16, 9, 0),
                dec!(-30),
            ),
        ];
        let equity = make_multi_day_equity(&[(15, 100000.0), (16, 99960.0), (17, 99930.0)]);
        let stats = compute_stats(&trades, &equity, dec!(100000));

        assert_eq!(stats.profit_factor, 0.0);
        assert_eq!(stats.max_consecutive_wins, 0);
        assert_eq!(stats.max_consecutive_losses, 2);
    }

    // -- Drawdown --

    #[test]
    fn test_max_drawdown_calculation() {
        // Equity: 100000 -> 100100 -> 100050 -> 100200
        // Peak at 100100, trough at 100050 -> DD = 50
        // Peak at 100200 (new peak) -> no new DD
        let equity =
            make_equity_curve(&[(0, 100000.0), (1, 100100.0), (2, 100050.0), (3, 100200.0)]);
        let (dd, dd_pct) = compute_max_drawdown(&equity);
        assert_eq!(dd, dec!(50));
        assert!((dd_pct - 50.0 / 100100.0).abs() < 1e-10);
    }

    #[test]
    fn test_max_drawdown_no_drawdown() {
        // Monotonically increasing equity
        let equity = make_equity_curve(&[(0, 100000.0), (1, 100050.0), (2, 100100.0)]);
        let (dd, dd_pct) = compute_max_drawdown(&equity);
        assert_eq!(dd, Decimal::ZERO);
        assert_eq!(dd_pct, 0.0);
    }

    #[test]
    fn test_max_drawdown_single_point() {
        let equity = make_equity_curve(&[(0, 100000.0)]);
        let (dd, dd_pct) = compute_max_drawdown(&equity);
        assert_eq!(dd, Decimal::ZERO);
        assert_eq!(dd_pct, 0.0);
    }

    // -- Consecutive streaks --

    #[test]
    fn test_consecutive_streaks() {
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                utc(2024, 1, 15, 8, 30),
                utc(2024, 1, 15, 10, 0),
                dec!(50),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16030),
                utc(2024, 1, 16, 8, 30),
                utc(2024, 1, 16, 10, 0),
                dec!(30),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16020),
                utc(2024, 1, 17, 8, 30),
                utc(2024, 1, 17, 10, 0),
                dec!(20),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(15960),
                utc(2024, 1, 18, 8, 30),
                utc(2024, 1, 18, 9, 0),
                dec!(-40),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(15970),
                utc(2024, 1, 19, 8, 30),
                utc(2024, 1, 19, 9, 0),
                dec!(-30),
            ),
        ];
        let (wins, losses) = compute_streaks(&trades);
        assert_eq!(wins, 3);
        assert_eq!(losses, 2);
    }

    #[test]
    fn test_break_even_resets_streaks() {
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                utc(2024, 1, 15, 8, 30),
                utc(2024, 1, 15, 10, 0),
                dec!(50),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16000),
                utc(2024, 1, 16, 8, 30),
                utc(2024, 1, 16, 10, 0),
                dec!(0),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                utc(2024, 1, 17, 8, 30),
                utc(2024, 1, 17, 10, 0),
                dec!(50),
            ),
        ];
        let (wins, losses) = compute_streaks(&trades);
        // Break-even resets streak, so max consecutive wins = 1
        assert_eq!(wins, 1);
        assert_eq!(losses, 0);
    }

    // -- Duration --

    #[test]
    fn test_avg_trade_duration() {
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                utc(2024, 1, 15, 8, 30),
                utc(2024, 1, 15, 10, 0),
                dec!(50),
            ), // 90 min
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(15960),
                utc(2024, 1, 16, 8, 30),
                utc(2024, 1, 16, 9, 0),
                dec!(-40),
            ), // 30 min
        ];
        let avg = compute_avg_duration(&trades);
        assert!((avg - 60.0).abs() < 1e-10); // (90 + 30) / 2 = 60
    }

    // -- Sharpe ratio --

    #[test]
    fn test_sharpe_ratio_with_insufficient_data() {
        assert_eq!(compute_sharpe(&[]), 0.0);
        assert_eq!(compute_sharpe(&[0.01]), 0.0);
    }

    #[test]
    fn test_sharpe_ratio_zero_std_dev() {
        let returns = vec![0.001, 0.001, 0.001, 0.001];
        // All returns equal -> std_dev = 0 -> Sharpe = 0
        assert_eq!(compute_sharpe(&returns), 0.0);
    }

    #[test]
    fn test_sharpe_ratio_positive() {
        let returns = vec![0.01, 0.02, -0.005, 0.015, 0.008];
        let sharpe = compute_sharpe(&returns);
        assert!(
            sharpe > 0.0,
            "Sharpe should be positive for net-positive returns"
        );
    }

    // -- Sortino ratio --

    #[test]
    fn test_sortino_ratio_no_negative_returns() {
        let returns = vec![0.01, 0.02, 0.015, 0.008];
        // No negative returns -> downside_dev = 0 -> Sortino = 0
        assert_eq!(compute_sortino(&returns), 0.0);
    }

    #[test]
    fn test_sortino_ratio_positive() {
        let returns = vec![0.01, -0.005, 0.02, -0.003, 0.015];
        let sortino = compute_sortino(&returns);
        assert!(
            sortino > 0.0,
            "Sortino should be positive for net-positive returns"
        );
    }

    // -- Daily returns computation --

    #[test]
    fn test_daily_returns_basic() {
        let equity = make_multi_day_equity(&[(15, 100000.0), (16, 100100.0), (17, 100200.0)]);
        let returns = compute_daily_returns(&equity);
        assert_eq!(returns.len(), 2);
        assert!((returns[0] - 0.001).abs() < 1e-10); // 100/100000
        assert!((returns[1] - (100.0 / 100100.0)).abs() < 1e-10);
    }

    #[test]
    fn test_daily_returns_empty_curve() {
        let returns = compute_daily_returns(&[]);
        assert!(returns.is_empty());
    }

    #[test]
    fn test_daily_returns_single_point() {
        let equity = make_equity_curve(&[(0, 100000.0)]);
        let returns = compute_daily_returns(&equity);
        assert!(returns.is_empty());
    }

    // -- Calmar ratio --

    #[test]
    fn test_calmar_ratio_no_drawdown() {
        let equity = make_multi_day_equity(&[(15, 100000.0), (16, 100100.0)]);
        let calmar = compute_calmar(&equity, dec!(100000), Decimal::ZERO);
        assert_eq!(calmar, 0.0);
    }

    #[test]
    fn test_calmar_ratio_with_drawdown() {
        let equity =
            make_multi_day_equity(&[(1, 100000.0), (15, 100050.0), (20, 99950.0), (31, 100200.0)]);
        let (dd, _) = compute_max_drawdown(&equity);
        let calmar = compute_calmar(&equity, dec!(100000), dd);
        // Should be a finite positive number
        assert!(calmar > 0.0);
        assert!(calmar.is_finite());
    }

    // -- Serde roundtrip --

    #[test]
    fn test_stats_serde_roundtrip() {
        let stats = BacktestStats::empty();
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: BacktestStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_trades, 0);
        assert_eq!(parsed.win_rate, 0.0);
        assert_eq!(parsed.total_pnl, Decimal::ZERO);
    }

    // -- Direction breakdown --

    #[test]
    fn test_direction_breakdown_mixed() {
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                utc(2024, 1, 15, 8, 30),
                utc(2024, 1, 15, 10, 0),
                dec!(50),
            ),
            make_trade(
                Direction::Short,
                dec!(16000),
                dec!(15950),
                utc(2024, 1, 16, 8, 30),
                utc(2024, 1, 16, 10, 0),
                dec!(50),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(15960),
                utc(2024, 1, 17, 8, 30),
                utc(2024, 1, 17, 9, 0),
                dec!(-40),
            ),
        ];
        let (lc, sc, lp, sp) = compute_direction_breakdown(&trades);
        assert_eq!(lc, 2);
        assert_eq!(sc, 1);
        assert_eq!(lp, dec!(10));
        assert_eq!(sp, dec!(50));
    }

    // -- Partition --

    #[test]
    fn test_partition_excludes_break_even() {
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                utc(2024, 1, 15, 8, 30),
                utc(2024, 1, 15, 10, 0),
                dec!(50),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16000),
                utc(2024, 1, 16, 8, 30),
                utc(2024, 1, 16, 10, 0),
                dec!(0),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(15960),
                utc(2024, 1, 17, 8, 30),
                utc(2024, 1, 17, 9, 0),
                dec!(-40),
            ),
        ];
        let (winners, losers) = partition_trades(&trades);
        assert_eq!(winners.len(), 1);
        assert_eq!(losers.len(), 1);
    }
}
