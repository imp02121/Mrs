//! Backtest result types: equity curve, daily PnL, and overall result container.
//!
//! These types are produced by [`super::engine::run_backtest`] and carry the
//! complete output of a backtest run: every trade, the equity curve over time,
//! and daily profit-and-loss records.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::Instrument;
use crate::strategy::config::StrategyConfig;
use crate::strategy::trade::Trade;

use super::stats::{BacktestStats, compute_stats};

/// A single point on the equity curve.
///
/// Represents the account equity immediately after a trade is closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    /// Timestamp of the equity snapshot (matches the trade exit time).
    pub timestamp: DateTime<Utc>,
    /// Account equity at this point.
    pub equity: Decimal,
}

/// Profit and loss for a single trading day.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyPnl {
    /// The trading date.
    pub date: NaiveDate,
    /// Net PnL for this day (sum of all trade PnLs including adds, costs).
    pub pnl: Decimal,
    /// Cumulative PnL from the start of the backtest through this day.
    pub cumulative: Decimal,
}

/// The complete result of a backtest run.
///
/// Contains every completed trade, the equity curve, daily PnL breakdown,
/// the configuration used, and computed statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    /// The instrument that was backtested.
    pub instrument: Instrument,
    /// The configuration used for this backtest.
    pub config: StrategyConfig,
    /// All completed trades in chronological order.
    pub trades: Vec<Trade>,
    /// Equity curve: one point per trade exit.
    pub equity_curve: Vec<EquityPoint>,
    /// Daily PnL: one entry per trading day that had at least one trade.
    pub daily_pnl: Vec<DailyPnl>,
    /// Computed statistics (win rate, Sharpe, drawdown, etc.).
    pub stats: BacktestStats,
}

impl BacktestResult {
    /// Build a `BacktestResult` from completed trades and config.
    ///
    /// Computes the equity curve and daily PnL from the trade sequence.
    /// Trades must already be sorted by exit time (as produced by the
    /// backtest engine).
    #[must_use]
    pub fn from_trades(instrument: Instrument, config: StrategyConfig, trades: Vec<Trade>) -> Self {
        let equity_curve = build_equity_curve(&trades, config.initial_capital);
        let daily_pnl = build_daily_pnl(&trades);
        let stats = compute_stats(&trades, &equity_curve, config.initial_capital);

        Self {
            instrument,
            config,
            trades,
            equity_curve,
            daily_pnl,
            stats,
        }
    }

    /// Total number of trades.
    #[must_use]
    pub fn trade_count(&self) -> usize {
        self.trades.len()
    }

    /// Final equity (initial capital + sum of all trade PnLs).
    #[must_use]
    pub fn final_equity(&self) -> Decimal {
        self.equity_curve
            .last()
            .map(|p| p.equity)
            .unwrap_or(self.config.initial_capital)
    }

    /// Total net PnL across all trades.
    #[must_use]
    pub fn total_pnl(&self) -> Decimal {
        self.trades.iter().map(|t| t.pnl_with_adds).sum()
    }
}

/// Build the equity curve from trades and initial capital.
///
/// Each point represents equity immediately after a trade closes.
fn build_equity_curve(trades: &[Trade], initial_capital: Decimal) -> Vec<EquityPoint> {
    let mut equity = initial_capital;
    let mut curve = Vec::with_capacity(trades.len());

    for trade in trades {
        equity += trade.pnl_with_adds;
        curve.push(EquityPoint {
            timestamp: trade.exit_time,
            equity,
        });
    }

    curve
}

/// Build the daily PnL breakdown from trades.
///
/// Groups trades by their exit date and sums PnL for each day.
fn build_daily_pnl(trades: &[Trade]) -> Vec<DailyPnl> {
    if trades.is_empty() {
        return Vec::new();
    }

    let mut daily: Vec<DailyPnl> = Vec::new();
    let mut cumulative = Decimal::ZERO;

    for trade in trades {
        let date = trade.exit_time.date_naive();
        cumulative += trade.pnl_with_adds;

        if let Some(last) = daily.last_mut()
            && last.date == date
        {
            last.pnl += trade.pnl_with_adds;
            last.cumulative = cumulative;
            continue;
        }

        daily.push(DailyPnl {
            date,
            pnl: trade.pnl_with_adds,
            cumulative,
        });
    }

    daily
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Instrument;
    use crate::strategy::config::StrategyConfig;
    use crate::strategy::types::{Direction, PositionStatus};
    use crate::test_helpers::utc;
    use rust_decimal_macros::dec;

    fn make_trade(
        direction: Direction,
        entry_price: Decimal,
        exit_price: Decimal,
        pnl_with_adds: Decimal,
        exit_time: DateTime<Utc>,
    ) -> Trade {
        Trade {
            instrument: Instrument::Dax,
            direction,
            entry_price,
            entry_time: utc(2024, 1, 15, 8, 30),
            exit_price,
            exit_time,
            stop_loss: dec!(15960),
            exit_reason: PositionStatus::EndOfDay,
            pnl_points: pnl_with_adds,
            pnl_with_adds,
            adds: Vec::new(),
            size: dec!(1),
        }
    }

    #[test]
    fn test_empty_backtest_result() {
        let config = StrategyConfig::default();
        let result = BacktestResult::from_trades(Instrument::Dax, config.clone(), Vec::new());
        assert_eq!(result.trade_count(), 0);
        assert_eq!(result.final_equity(), config.initial_capital);
        assert_eq!(result.total_pnl(), dec!(0));
        assert!(result.equity_curve.is_empty());
        assert!(result.daily_pnl.is_empty());
    }

    #[test]
    fn test_single_winning_trade() {
        let config = StrategyConfig {
            initial_capital: dec!(100000),
            ..StrategyConfig::default()
        };
        let trades = vec![make_trade(
            Direction::Long,
            dec!(16000),
            dec!(16050),
            dec!(50),
            utc(2024, 1, 15, 16, 30),
        )];
        let result = BacktestResult::from_trades(Instrument::Dax, config, trades);
        assert_eq!(result.trade_count(), 1);
        assert_eq!(result.final_equity(), dec!(100050));
        assert_eq!(result.total_pnl(), dec!(50));
        assert_eq!(result.equity_curve.len(), 1);
        assert_eq!(result.daily_pnl.len(), 1);
        assert_eq!(result.daily_pnl[0].pnl, dec!(50));
        assert_eq!(result.daily_pnl[0].cumulative, dec!(50));
    }

    #[test]
    fn test_multiple_trades_same_day() {
        let config = StrategyConfig {
            initial_capital: dec!(100000),
            ..StrategyConfig::default()
        };
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                dec!(50),
                utc(2024, 1, 15, 10, 0),
            ),
            make_trade(
                Direction::Short,
                dec!(16050),
                dec!(16000),
                dec!(50),
                utc(2024, 1, 15, 16, 30),
            ),
        ];
        let result = BacktestResult::from_trades(Instrument::Dax, config, trades);
        assert_eq!(result.trade_count(), 2);
        assert_eq!(result.final_equity(), dec!(100100));
        assert_eq!(result.daily_pnl.len(), 1);
        assert_eq!(result.daily_pnl[0].pnl, dec!(100));
    }

    #[test]
    fn test_multiple_trades_different_days() {
        let config = StrategyConfig {
            initial_capital: dec!(100000),
            ..StrategyConfig::default()
        };
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                dec!(50),
                utc(2024, 1, 15, 16, 30),
            ),
            make_trade(
                Direction::Long,
                dec!(16100),
                dec!(16060),
                dec!(-40),
                utc(2024, 1, 16, 16, 30),
            ),
        ];
        let result = BacktestResult::from_trades(Instrument::Dax, config, trades);
        assert_eq!(result.daily_pnl.len(), 2);
        assert_eq!(result.daily_pnl[0].pnl, dec!(50));
        assert_eq!(result.daily_pnl[0].cumulative, dec!(50));
        assert_eq!(result.daily_pnl[1].pnl, dec!(-40));
        assert_eq!(result.daily_pnl[1].cumulative, dec!(10));
        assert_eq!(result.final_equity(), dec!(100010));
    }

    #[test]
    fn test_equity_curve_monotonic_with_wins() {
        let config = StrategyConfig {
            initial_capital: dec!(100000),
            ..StrategyConfig::default()
        };
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(16050),
                dec!(50),
                utc(2024, 1, 15, 16, 30),
            ),
            make_trade(
                Direction::Long,
                dec!(16100),
                dec!(16200),
                dec!(100),
                utc(2024, 1, 16, 16, 30),
            ),
        ];
        let result = BacktestResult::from_trades(Instrument::Dax, config, trades);
        assert_eq!(result.equity_curve[0].equity, dec!(100050));
        assert_eq!(result.equity_curve[1].equity, dec!(100150));
    }

    #[test]
    fn test_backtest_result_serde_roundtrip() {
        let config = StrategyConfig::default();
        let trades = vec![make_trade(
            Direction::Long,
            dec!(16000),
            dec!(16050),
            dec!(50),
            utc(2024, 1, 15, 16, 30),
        )];
        let result = BacktestResult::from_trades(Instrument::Dax, config, trades);
        let json = serde_json::to_string(&result).unwrap();
        let parsed: BacktestResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.trade_count(), 1);
        assert_eq!(parsed.instrument, Instrument::Dax);
    }

    #[test]
    fn test_losing_trades_decrease_equity() {
        let config = StrategyConfig {
            initial_capital: dec!(100000),
            ..StrategyConfig::default()
        };
        let trades = vec![
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(15960),
                dec!(-40),
                utc(2024, 1, 15, 9, 0),
            ),
            make_trade(
                Direction::Long,
                dec!(16000),
                dec!(15960),
                dec!(-40),
                utc(2024, 1, 16, 9, 0),
            ),
        ];
        let result = BacktestResult::from_trades(Instrument::Dax, config, trades);
        assert_eq!(result.final_equity(), dec!(99920));
        assert_eq!(result.total_pnl(), dec!(-80));
    }
}
