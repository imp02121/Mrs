//! Trade database queries.
//!
//! Provides bulk insert and paginated queries for the `trades` table.
//! Trades belong to a backtest run and are cascade-deleted when the run is removed.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::error::DbError;

/// Maximum rows per INSERT batch (14 params each = 14000, under 65535).
const BATCH_SIZE: usize = 1000;

/// A row from the `trades` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct TradeRow {
    /// Unique trade identifier.
    pub id: Uuid,
    /// Parent backtest run.
    pub backtest_run_id: Uuid,
    /// Foreign key to instruments.
    pub instrument_id: i16,
    /// Trade direction ("Long" or "Short").
    pub direction: String,
    /// Entry fill price.
    pub entry_price: Decimal,
    /// Entry fill time.
    pub entry_time: DateTime<Utc>,
    /// Exit fill price.
    pub exit_price: Decimal,
    /// Exit fill time.
    pub exit_time: DateTime<Utc>,
    /// Stop loss level.
    pub stop_loss: Decimal,
    /// Why the position was closed.
    pub exit_reason: String,
    /// PnL in points (base position only).
    pub pnl_points: Decimal,
    /// Total PnL including add-on positions.
    pub pnl_with_adds: Decimal,
    /// Add-on position details as JSONB.
    pub adds: serde_json::Value,
    /// Calendar date of the trade.
    pub trade_date: NaiveDate,
}

/// Parameters for inserting a single trade.
#[derive(Debug, Clone)]
pub struct InsertTrade {
    /// Parent backtest run.
    pub backtest_run_id: Uuid,
    /// Instrument database ID.
    pub instrument_id: i16,
    /// Trade direction ("Long" or "Short").
    pub direction: String,
    /// Entry fill price.
    pub entry_price: Decimal,
    /// Entry fill time.
    pub entry_time: DateTime<Utc>,
    /// Exit fill price.
    pub exit_price: Decimal,
    /// Exit fill time.
    pub exit_time: DateTime<Utc>,
    /// Stop loss level.
    pub stop_loss: Decimal,
    /// Why the position was closed.
    pub exit_reason: String,
    /// PnL in points (base position only).
    pub pnl_points: Decimal,
    /// Total PnL including add-on positions.
    pub pnl_with_adds: Decimal,
    /// Add-on position details as JSONB.
    pub adds: serde_json::Value,
    /// Calendar date of the trade.
    pub trade_date: NaiveDate,
}

/// Bulk insert trades in batches of 1000.
///
/// Returns the total number of rows inserted.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn insert_trades(pool: &PgPool, trades: &[InsertTrade]) -> Result<usize, DbError> {
    if trades.is_empty() {
        return Ok(0);
    }

    let mut total = 0usize;
    for chunk in trades.chunks(BATCH_SIZE) {
        total += insert_trade_batch(pool, chunk).await?;
    }
    Ok(total)
}

/// Fetch trades for a backtest run with pagination.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn get_trades_for_run(
    pool: &PgPool,
    run_id: Uuid,
    page: i64,
    per_page: i64,
) -> Result<Vec<TradeRow>, DbError> {
    let offset = page.saturating_mul(per_page);
    let rows = sqlx::query_as::<_, TradeRow>(
        r#"
        SELECT id, backtest_run_id, instrument_id, direction,
               entry_price, entry_time, exit_price, exit_time,
               stop_loss, exit_reason, pnl_points, pnl_with_adds,
               adds, trade_date
        FROM trades
        WHERE backtest_run_id = $1
        ORDER BY entry_time ASC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(run_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Count trades belonging to a backtest run.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn count_trades_for_run(pool: &PgPool, run_id: Uuid) -> Result<i64, DbError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM trades WHERE backtest_run_id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Insert a single batch of trades.
async fn insert_trade_batch(pool: &PgPool, batch: &[InsertTrade]) -> Result<usize, DbError> {
    let mut sql = String::from(
        "INSERT INTO trades (backtest_run_id, instrument_id, direction, \
         entry_price, entry_time, exit_price, exit_time, stop_loss, \
         exit_reason, pnl_points, pnl_with_adds, adds, trade_date) VALUES ",
    );

    let params_per_row = 13;
    for (i, _) in batch.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        let base = i * params_per_row + 1;
        sql.push_str(&format!(
            "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
            base,
            base + 1,
            base + 2,
            base + 3,
            base + 4,
            base + 5,
            base + 6,
            base + 7,
            base + 8,
            base + 9,
            base + 10,
            base + 11,
            base + 12,
        ));
    }

    let mut query = sqlx::query(&sql);
    for trade in batch {
        query = query
            .bind(trade.backtest_run_id)
            .bind(trade.instrument_id)
            .bind(&trade.direction)
            .bind(trade.entry_price)
            .bind(trade.entry_time)
            .bind(trade.exit_price)
            .bind(trade.exit_time)
            .bind(trade.stop_loss)
            .bind(&trade.exit_reason)
            .bind(trade.pnl_points)
            .bind(trade.pnl_with_adds)
            .bind(&trade.adds)
            .bind(trade.trade_date);
    }

    let result = query.execute(pool).await?;
    Ok(result.rows_affected() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_size_within_postgres_limits() {
        const { assert!(BATCH_SIZE * 13 < 65535) };
    }

    #[test]
    fn test_trade_row_construction() {
        let row = TradeRow {
            id: Uuid::new_v4(),
            backtest_run_id: Uuid::new_v4(),
            instrument_id: 1,
            direction: "Long".into(),
            entry_price: Decimal::new(1600000, 2),
            entry_time: Utc::now(),
            exit_price: Decimal::new(1605000, 2),
            exit_time: Utc::now(),
            stop_loss: Decimal::new(1596000, 2),
            exit_reason: "EndOfDay".into(),
            pnl_points: Decimal::new(5000, 2),
            pnl_with_adds: Decimal::new(5000, 2),
            adds: serde_json::json!([]),
            trade_date: NaiveDate::from_ymd_opt(2024, 1, 15).expect("valid date"),
        };
        assert_eq!(row.direction, "Long");
    }

    #[test]
    fn test_trade_row_serde_roundtrip() {
        let row = TradeRow {
            id: Uuid::new_v4(),
            backtest_run_id: Uuid::new_v4(),
            instrument_id: 1,
            direction: "Short".into(),
            entry_price: Decimal::new(1600000, 2),
            entry_time: Utc::now(),
            exit_price: Decimal::new(1596000, 2),
            exit_time: Utc::now(),
            stop_loss: Decimal::new(1604000, 2),
            exit_reason: "StopLoss".into(),
            pnl_points: Decimal::new(4000, 2),
            pnl_with_adds: Decimal::new(4000, 2),
            adds: serde_json::json!([]),
            trade_date: NaiveDate::from_ymd_opt(2024, 1, 15).expect("valid date"),
        };
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: TradeRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.direction, "Short");
        assert_eq!(parsed.id, row.id);
    }

    #[test]
    fn test_insert_trade_long_direction() {
        let trade = InsertTrade {
            backtest_run_id: Uuid::new_v4(),
            instrument_id: 1,
            direction: "Long".into(),
            entry_price: Decimal::new(1600000, 2),
            entry_time: Utc::now(),
            exit_price: Decimal::new(1605000, 2),
            exit_time: Utc::now(),
            stop_loss: Decimal::new(1596000, 2),
            exit_reason: "TakeProfit".into(),
            pnl_points: Decimal::new(5000, 2),
            pnl_with_adds: Decimal::new(5000, 2),
            adds: serde_json::json!([]),
            trade_date: NaiveDate::from_ymd_opt(2024, 3, 15).expect("valid date"),
        };
        assert_eq!(trade.direction, "Long");
        assert!(trade.pnl_points > Decimal::ZERO);
    }

    #[test]
    fn test_insert_trade_short_direction() {
        let trade = InsertTrade {
            backtest_run_id: Uuid::new_v4(),
            instrument_id: 2,
            direction: "Short".into(),
            entry_price: Decimal::new(1605000, 2),
            entry_time: Utc::now(),
            exit_price: Decimal::new(1600000, 2),
            exit_time: Utc::now(),
            stop_loss: Decimal::new(1609000, 2),
            exit_reason: "EndOfDay".into(),
            pnl_points: Decimal::new(5000, 2),
            pnl_with_adds: Decimal::new(5000, 2),
            adds: serde_json::json!([]),
            trade_date: NaiveDate::from_ymd_opt(2024, 3, 15).expect("valid date"),
        };
        assert_eq!(trade.direction, "Short");
        assert_eq!(trade.instrument_id, 2);
    }

    #[test]
    fn test_insert_trade_with_adds_json_array() {
        let adds = serde_json::json!([
            {"price": 16020.50, "time": "2024-01-15T10:00:00Z", "size": 0.5},
            {"price": 16040.00, "time": "2024-01-15T11:00:00Z", "size": 0.5}
        ]);
        let trade = InsertTrade {
            backtest_run_id: Uuid::new_v4(),
            instrument_id: 1,
            direction: "Long".into(),
            entry_price: Decimal::new(1600000, 2),
            entry_time: Utc::now(),
            exit_price: Decimal::new(1605000, 2),
            exit_time: Utc::now(),
            stop_loss: Decimal::new(1596000, 2),
            exit_reason: "TakeProfit".into(),
            pnl_points: Decimal::new(5000, 2),
            pnl_with_adds: Decimal::new(7500, 2),
            adds,
            trade_date: NaiveDate::from_ymd_opt(2024, 1, 15).expect("valid date"),
        };
        assert!(trade.adds.is_array());
        assert_eq!(trade.adds.as_array().unwrap().len(), 2);
        assert!(trade.pnl_with_adds > trade.pnl_points);
    }

    #[test]
    fn test_trade_batch_size_constant() {
        // 13 params per trade row, 1000 rows = 13000 < 65535
        assert_eq!(BATCH_SIZE, 1000);
        assert!(BATCH_SIZE * 13 < 65535);
    }

    #[test]
    fn test_insert_trade_clone() {
        let trade = InsertTrade {
            backtest_run_id: Uuid::new_v4(),
            instrument_id: 1,
            direction: "Long".into(),
            entry_price: Decimal::new(1600000, 2),
            entry_time: Utc::now(),
            exit_price: Decimal::new(1605000, 2),
            exit_time: Utc::now(),
            stop_loss: Decimal::new(1596000, 2),
            exit_reason: "EndOfDay".into(),
            pnl_points: Decimal::new(5000, 2),
            pnl_with_adds: Decimal::new(5000, 2),
            adds: serde_json::json!([]),
            trade_date: NaiveDate::from_ymd_opt(2024, 1, 15).expect("valid date"),
        };
        let cloned = trade.clone();
        assert_eq!(cloned.backtest_run_id, trade.backtest_run_id);
        assert_eq!(cloned.direction, trade.direction);
    }
}
