//! Backtest report serialization and summary formatting.
//!
//! Provides helper functions for converting [`BacktestResult`] to JSON
//! and generating human-readable summary text for display in the CLI
//! or Telegram notifications.

use std::fmt;

use rust_decimal::Decimal;
use serde::Serialize;

use super::result::BacktestResult;
use super::stats::BacktestStats;

/// Serialize a [`BacktestResult`] to a pretty-printed JSON string.
///
/// # Errors
///
/// Returns a `serde_json::Error` if serialization fails (should not occur
/// for well-formed types).
pub fn to_json(result: &BacktestResult) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(result)
}

/// Serialize a [`BacktestResult`] to a compact JSON string.
///
/// # Errors
///
/// Returns a `serde_json::Error` if serialization fails.
pub fn to_json_compact(result: &BacktestResult) -> Result<String, serde_json::Error> {
    serde_json::to_string(result)
}

/// A condensed summary of a backtest run, suitable for display or logging.
#[derive(Debug, Clone, Serialize)]
pub struct BacktestSummary {
    /// Instrument ticker (e.g. "DAX").
    pub instrument: String,
    /// Date range of the backtest.
    pub date_range: String,
    /// Total number of trades.
    pub total_trades: u32,
    /// Win rate as a percentage string (e.g. "65.2%").
    pub win_rate: String,
    /// Net PnL.
    pub total_pnl: Decimal,
    /// Profit factor.
    pub profit_factor: String,
    /// Max drawdown as absolute value.
    pub max_drawdown: Decimal,
    /// Max drawdown as percentage string (e.g. "3.5%").
    pub max_drawdown_pct: String,
    /// Sharpe ratio.
    pub sharpe_ratio: String,
    /// Sortino ratio.
    pub sortino_ratio: String,
    /// Calmar ratio.
    pub calmar_ratio: String,
}

impl BacktestSummary {
    /// Create a summary from a backtest result.
    #[must_use]
    pub fn from_result(result: &BacktestResult) -> Self {
        let stats = &result.stats;
        Self {
            instrument: result.instrument.ticker().to_owned(),
            date_range: format!("{} to {}", result.config.date_from, result.config.date_to),
            total_trades: stats.total_trades,
            win_rate: format_pct(stats.win_rate),
            total_pnl: stats.total_pnl,
            profit_factor: format_ratio(stats.profit_factor),
            max_drawdown: stats.max_drawdown,
            max_drawdown_pct: format_pct(stats.max_drawdown_pct),
            sharpe_ratio: format_ratio(stats.sharpe_ratio),
            sortino_ratio: format_ratio(stats.sortino_ratio),
            calmar_ratio: format_ratio(stats.calmar_ratio),
        }
    }
}

impl fmt::Display for BacktestSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Backtest Summary: {} ({})",
            self.instrument, self.date_range
        )?;
        writeln!(f, "  Trades:         {}", self.total_trades)?;
        writeln!(f, "  Win Rate:       {}", self.win_rate)?;
        writeln!(f, "  Total PnL:      {}", self.total_pnl)?;
        writeln!(f, "  Profit Factor:  {}", self.profit_factor)?;
        writeln!(
            f,
            "  Max Drawdown:   {} ({})",
            self.max_drawdown, self.max_drawdown_pct
        )?;
        writeln!(f, "  Sharpe Ratio:   {}", self.sharpe_ratio)?;
        writeln!(f, "  Sortino Ratio:  {}", self.sortino_ratio)?;
        write!(f, "  Calmar Ratio:   {}", self.calmar_ratio)
    }
}

impl fmt::Display for BacktestStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Trades: {} (W:{} L:{})",
            self.total_trades, self.winning_trades, self.losing_trades
        )?;
        writeln!(f, "Win Rate: {}", format_pct(self.win_rate))?;
        writeln!(
            f,
            "PnL: {} (Avg Win: {} / Avg Loss: {})",
            self.total_pnl, self.avg_win, self.avg_loss
        )?;
        writeln!(
            f,
            "Largest Win: {} / Largest Loss: {}",
            self.largest_win, self.largest_loss
        )?;
        writeln!(f, "Profit Factor: {}", format_ratio(self.profit_factor))?;
        writeln!(
            f,
            "Max Drawdown: {} ({})",
            self.max_drawdown,
            format_pct(self.max_drawdown_pct)
        )?;
        writeln!(
            f,
            "Sharpe: {} / Sortino: {} / Calmar: {}",
            format_ratio(self.sharpe_ratio),
            format_ratio(self.sortino_ratio),
            format_ratio(self.calmar_ratio),
        )?;
        writeln!(
            f,
            "Consecutive: W:{} L:{}",
            self.max_consecutive_wins, self.max_consecutive_losses
        )?;
        writeln!(
            f,
            "Avg Duration: {:.1} min",
            self.avg_trade_duration_minutes
        )?;
        write!(
            f,
            "Long: {} ({}) / Short: {} ({})",
            self.long_trades, self.long_pnl, self.short_trades, self.short_pnl,
        )
    }
}

/// Format a ratio (Sharpe, profit factor, etc.) for display.
///
/// Infinite values display as "Inf", NaN as "N/A".
fn format_ratio(value: f64) -> String {
    if value.is_infinite() {
        "Inf".to_owned()
    } else if value.is_nan() {
        "N/A".to_owned()
    } else {
        format!("{value:.2}")
    }
}

/// Format a fraction as a percentage string (e.g. 0.652 -> "65.20%").
fn format_pct(value: f64) -> String {
    if value.is_nan() {
        "N/A".to_owned()
    } else {
        format!("{:.2}%", value * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ratio_normal() {
        assert_eq!(format_ratio(1.5), "1.50");
        assert_eq!(format_ratio(-0.3), "-0.30");
        assert_eq!(format_ratio(0.0), "0.00");
    }

    #[test]
    fn test_format_ratio_infinite() {
        assert_eq!(format_ratio(f64::INFINITY), "Inf");
        assert_eq!(format_ratio(f64::NEG_INFINITY), "Inf");
    }

    #[test]
    fn test_format_ratio_nan() {
        assert_eq!(format_ratio(f64::NAN), "N/A");
    }

    #[test]
    fn test_format_pct_normal() {
        assert_eq!(format_pct(0.652), "65.20%");
        assert_eq!(format_pct(1.0), "100.00%");
        assert_eq!(format_pct(0.0), "0.00%");
    }

    #[test]
    fn test_format_pct_nan() {
        assert_eq!(format_pct(f64::NAN), "N/A");
    }

    #[test]
    fn test_stats_display() {
        let stats = BacktestStats {
            total_trades: 10,
            winning_trades: 6,
            losing_trades: 4,
            win_rate: 0.6,
            total_pnl: Decimal::new(500, 0),
            avg_win: Decimal::new(150, 0),
            avg_loss: Decimal::new(-100, 0),
            largest_win: Decimal::new(300, 0),
            largest_loss: Decimal::new(-200, 0),
            profit_factor: 2.25,
            max_drawdown: Decimal::new(300, 0),
            max_drawdown_pct: 0.03,
            sharpe_ratio: 1.5,
            sortino_ratio: 2.1,
            calmar_ratio: 3.0,
            max_consecutive_wins: 4,
            max_consecutive_losses: 2,
            avg_trade_duration_minutes: 120.5,
            long_trades: 5,
            short_trades: 5,
            long_pnl: Decimal::new(300, 0),
            short_pnl: Decimal::new(200, 0),
        };
        let display = format!("{stats}");
        assert!(display.contains("Trades: 10 (W:6 L:4)"));
        assert!(display.contains("60.00%"));
        assert!(display.contains("Profit Factor: 2.25"));
        assert!(display.contains("Sharpe: 1.50"));
        assert!(display.contains("Long: 5 (300)"));
    }

    #[test]
    fn test_format_neg_infinity() {
        assert_eq!(format_ratio(f64::NEG_INFINITY), "Inf");
    }
}
