//! Backtest run database queries.
//!
//! Provides insert, get, list, and delete for the `backtest_runs` table.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::error::DbError;

/// A row from the `backtest_runs` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct BacktestRunRow {
    /// Unique run identifier.
    pub id: Uuid,
    /// Foreign key to `strategy_configs`.
    pub config_id: Uuid,
    /// Foreign key to `instruments`.
    pub instrument_id: i16,
    /// Backtest start date (inclusive).
    pub start_date: NaiveDate,
    /// Backtest end date (inclusive).
    pub end_date: NaiveDate,
    /// Number of trades in this run.
    pub total_trades: i32,
    /// Summary statistics as JSONB.
    pub stats: serde_json::Value,
    /// Backtest wall-clock duration in milliseconds.
    pub duration_ms: i32,
    /// When this run was created.
    pub created_at: DateTime<Utc>,
}

/// Parameters for inserting a new backtest run.
#[derive(Debug, Clone)]
pub struct InsertBacktestRun {
    /// Strategy config used.
    pub config_id: Uuid,
    /// Instrument that was backtested.
    pub instrument_id: i16,
    /// Start date.
    pub start_date: NaiveDate,
    /// End date.
    pub end_date: NaiveDate,
    /// Number of completed trades.
    pub total_trades: i32,
    /// Stats JSONB.
    pub stats: serde_json::Value,
    /// Wall-clock duration in ms.
    pub duration_ms: i32,
}

/// Insert a new backtest run.
///
/// Returns the generated UUID.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn insert_backtest_run(pool: &PgPool, run: &InsertBacktestRun) -> Result<Uuid, DbError> {
    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO backtest_runs (config_id, instrument_id, start_date, end_date, total_trades, stats, duration_ms)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(run.config_id)
    .bind(run.instrument_id)
    .bind(run.start_date)
    .bind(run.end_date)
    .bind(run.total_trades)
    .bind(&run.stats)
    .bind(run.duration_ms)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Fetch a backtest run by ID.
///
/// # Errors
///
/// Returns [`DbError::NotFound`] if no run matches the ID.
/// Returns [`DbError::Database`] on SQL failure.
pub async fn get_backtest_run(pool: &PgPool, id: Uuid) -> Result<BacktestRunRow, DbError> {
    sqlx::query_as::<_, BacktestRunRow>(
        r#"
        SELECT id, config_id, instrument_id, start_date, end_date,
               total_trades, stats, duration_ms, created_at
        FROM backtest_runs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound(format!("backtest_run id={id}")))
}

/// List backtest runs with pagination, newest first.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn list_backtest_runs(
    pool: &PgPool,
    page: i64,
    per_page: i64,
) -> Result<Vec<BacktestRunRow>, DbError> {
    let offset = page.saturating_mul(per_page);
    let rows = sqlx::query_as::<_, BacktestRunRow>(
        r#"
        SELECT id, config_id, instrument_id, start_date, end_date,
               total_trades, stats, duration_ms, created_at
        FROM backtest_runs
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Delete a backtest run by ID.
///
/// Returns `true` if a row was deleted, `false` if it did not exist.
/// Associated trades are cascade-deleted automatically.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn delete_backtest_run(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query("DELETE FROM backtest_runs WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backtest_run_row_construction() {
        let row = BacktestRunRow {
            id: Uuid::new_v4(),
            config_id: Uuid::new_v4(),
            instrument_id: 1,
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date"),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).expect("valid date"),
            total_trades: 150,
            stats: serde_json::json!({"win_rate": 0.55}),
            duration_ms: 1234,
            created_at: Utc::now(),
        };
        assert_eq!(row.total_trades, 150);
        assert_eq!(row.instrument_id, 1);
    }

    #[test]
    fn test_backtest_run_row_serde_roundtrip() {
        let row = BacktestRunRow {
            id: Uuid::new_v4(),
            config_id: Uuid::new_v4(),
            instrument_id: 2,
            start_date: NaiveDate::from_ymd_opt(2024, 6, 1).expect("valid date"),
            end_date: NaiveDate::from_ymd_opt(2024, 6, 30).expect("valid date"),
            total_trades: 20,
            stats: serde_json::json!({}),
            duration_ms: 500,
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: BacktestRunRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.id, row.id);
        assert_eq!(parsed.total_trades, 20);
    }

    #[test]
    fn test_insert_params_construction() {
        let params = InsertBacktestRun {
            config_id: Uuid::new_v4(),
            instrument_id: 1,
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date"),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).expect("valid date"),
            total_trades: 100,
            stats: serde_json::json!({"profit_factor": 1.5}),
            duration_ms: 2000,
        };
        assert_eq!(params.total_trades, 100);
    }

    #[test]
    fn test_insert_backtest_run_with_full_stats() {
        let stats = serde_json::json!({
            "win_rate": 0.55,
            "profit_factor": 1.8,
            "max_drawdown": -0.12,
            "sharpe_ratio": 1.2,
            "total_pnl": 5432.50,
            "avg_trade_pnl": 36.22,
            "max_consecutive_losses": 5
        });
        let params = InsertBacktestRun {
            config_id: Uuid::new_v4(),
            instrument_id: 3,
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date"),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).expect("valid date"),
            total_trades: 150,
            stats,
            duration_ms: 3500,
        };
        assert_eq!(params.instrument_id, 3);
        assert_eq!(params.stats["win_rate"], 0.55);
        assert_eq!(params.stats["max_consecutive_losses"], 5);
    }

    #[test]
    fn test_insert_backtest_run_with_empty_stats() {
        let params = InsertBacktestRun {
            config_id: Uuid::new_v4(),
            instrument_id: 1,
            start_date: NaiveDate::from_ymd_opt(2024, 6, 1).expect("valid date"),
            end_date: NaiveDate::from_ymd_opt(2024, 6, 30).expect("valid date"),
            total_trades: 0,
            stats: serde_json::json!({}),
            duration_ms: 50,
        };
        assert_eq!(params.total_trades, 0);
        assert!(params.stats.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_pagination_offset_calculation() {
        // The list_backtest_runs function computes offset = page * per_page
        let page: i64 = 0;
        let per_page: i64 = 25;
        assert_eq!(page.saturating_mul(per_page), 0);

        let page: i64 = 1;
        assert_eq!(page.saturating_mul(per_page), 25);

        let page: i64 = 3;
        let per_page: i64 = 10;
        assert_eq!(page.saturating_mul(per_page), 30);
    }

    #[test]
    fn test_pagination_offset_saturating() {
        // Verify saturating_mul prevents overflow
        let page: i64 = i64::MAX;
        let per_page: i64 = 2;
        assert_eq!(page.saturating_mul(per_page), i64::MAX);
    }

    #[test]
    fn test_insert_backtest_run_clone() {
        let params = InsertBacktestRun {
            config_id: Uuid::new_v4(),
            instrument_id: 1,
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date"),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).expect("valid date"),
            total_trades: 42,
            stats: serde_json::json!({"win_rate": 0.6}),
            duration_ms: 1000,
        };
        let cloned = params.clone();
        assert_eq!(cloned.config_id, params.config_id);
        assert_eq!(cloned.total_trades, params.total_trades);
        assert_eq!(cloned.stats, params.stats);
    }

    #[test]
    fn test_backtest_run_row_date_range() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");
        let end = NaiveDate::from_ymd_opt(2024, 12, 31).expect("valid date");
        let row = BacktestRunRow {
            id: Uuid::new_v4(),
            config_id: Uuid::new_v4(),
            instrument_id: 1,
            start_date: start,
            end_date: end,
            total_trades: 100,
            stats: serde_json::json!({}),
            duration_ms: 500,
            created_at: Utc::now(),
        };
        assert!(row.end_date > row.start_date);
        assert_eq!((row.end_date - row.start_date).num_days(), 365);
    }
}
