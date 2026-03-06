//! Live signal database queries.
//!
//! Provides upsert and query operations for the `live_signals` table.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::error::DbError;

/// A row from the `live_signals` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct SignalRow {
    /// Unique signal identifier.
    pub id: Uuid,
    /// Foreign key to instruments.
    pub instrument_id: i16,
    /// Date this signal applies to.
    pub signal_date: NaiveDate,
    /// Signal bar high price.
    pub signal_bar_high: Decimal,
    /// Signal bar low price.
    pub signal_bar_low: Decimal,
    /// Buy stop level.
    pub buy_level: Decimal,
    /// Sell stop level.
    pub sell_level: Decimal,
    /// Signal status (e.g. "pending", "filled", "expired").
    pub status: String,
    /// Fill details as optional JSONB.
    pub fill_details: Option<serde_json::Value>,
    /// When the signal was created.
    pub created_at: DateTime<Utc>,
}

/// Parameters for upserting a live signal.
#[derive(Debug, Clone)]
pub struct UpsertSignal {
    /// Instrument database ID.
    pub instrument_id: i16,
    /// Signal date.
    pub signal_date: NaiveDate,
    /// Signal bar high.
    pub signal_bar_high: Decimal,
    /// Signal bar low.
    pub signal_bar_low: Decimal,
    /// Buy stop level.
    pub buy_level: Decimal,
    /// Sell stop level.
    pub sell_level: Decimal,
    /// Current status.
    pub status: String,
    /// Optional fill details.
    pub fill_details: Option<serde_json::Value>,
}

/// Upsert a live signal (insert or update on instrument_id + signal_date conflict).
///
/// Returns the signal UUID.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn upsert_signal(pool: &PgPool, signal: &UpsertSignal) -> Result<Uuid, DbError> {
    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO live_signals (instrument_id, signal_date, signal_bar_high, signal_bar_low,
                                  buy_level, sell_level, status, fill_details)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (instrument_id, signal_date) DO UPDATE SET
            signal_bar_high = EXCLUDED.signal_bar_high,
            signal_bar_low  = EXCLUDED.signal_bar_low,
            buy_level       = EXCLUDED.buy_level,
            sell_level      = EXCLUDED.sell_level,
            status          = EXCLUDED.status,
            fill_details    = EXCLUDED.fill_details
        RETURNING id
        "#,
    )
    .bind(signal.instrument_id)
    .bind(signal.signal_date)
    .bind(signal.signal_bar_high)
    .bind(signal.signal_bar_low)
    .bind(signal.buy_level)
    .bind(signal.sell_level)
    .bind(&signal.status)
    .bind(&signal.fill_details)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Get the most recent signal for a given instrument.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn get_latest_signal(
    pool: &PgPool,
    instrument_id: i16,
) -> Result<Option<SignalRow>, DbError> {
    let row = sqlx::query_as::<_, SignalRow>(
        r#"
        SELECT id, instrument_id, signal_date, signal_bar_high, signal_bar_low,
               buy_level, sell_level, status, fill_details, created_at
        FROM live_signals
        WHERE instrument_id = $1
        ORDER BY signal_date DESC
        LIMIT 1
        "#,
    )
    .bind(instrument_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Get all signals for today's date across all instruments.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn get_today_signals(pool: &PgPool) -> Result<Vec<SignalRow>, DbError> {
    let rows = sqlx::query_as::<_, SignalRow>(
        r#"
        SELECT id, instrument_id, signal_date, signal_bar_high, signal_bar_low,
               buy_level, sell_level, status, fill_details, created_at
        FROM live_signals
        WHERE signal_date = CURRENT_DATE
        ORDER BY instrument_id
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_row_construction() {
        let row = SignalRow {
            id: Uuid::new_v4(),
            instrument_id: 1,
            signal_date: NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date"),
            signal_bar_high: Decimal::new(1605000, 2),
            signal_bar_low: Decimal::new(1598000, 2),
            buy_level: Decimal::new(1605200, 2),
            sell_level: Decimal::new(1597800, 2),
            status: "pending".into(),
            fill_details: None,
            created_at: Utc::now(),
        };
        assert_eq!(row.status, "pending");
        assert_eq!(row.instrument_id, 1);
    }

    #[test]
    fn test_signal_row_serde_roundtrip() {
        let row = SignalRow {
            id: Uuid::new_v4(),
            instrument_id: 3,
            signal_date: NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date"),
            signal_bar_high: Decimal::new(1800000, 2),
            signal_bar_low: Decimal::new(1795000, 2),
            buy_level: Decimal::new(1800200, 2),
            sell_level: Decimal::new(1794800, 2),
            status: "filled".into(),
            fill_details: Some(serde_json::json!({"price": 18002})),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: SignalRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.status, "filled");
        assert!(parsed.fill_details.is_some());
    }

    #[test]
    fn test_upsert_signal_construction() {
        let signal = UpsertSignal {
            instrument_id: 1,
            signal_date: NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date"),
            signal_bar_high: Decimal::new(1605000, 2),
            signal_bar_low: Decimal::new(1598000, 2),
            buy_level: Decimal::new(1605200, 2),
            sell_level: Decimal::new(1597800, 2),
            status: "pending".into(),
            fill_details: None,
        };
        assert_eq!(signal.instrument_id, 1);
    }

    #[test]
    fn test_upsert_signal_with_fill_details() {
        let fill = serde_json::json!({
            "fill_price": 16052.00,
            "fill_time": "2024-06-15T09:30:00Z",
            "direction": "Long",
            "slippage": 0.20
        });
        let signal = UpsertSignal {
            instrument_id: 1,
            signal_date: NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date"),
            signal_bar_high: Decimal::new(1605000, 2),
            signal_bar_low: Decimal::new(1598000, 2),
            buy_level: Decimal::new(1605200, 2),
            sell_level: Decimal::new(1597800, 2),
            status: "filled".into(),
            fill_details: Some(fill.clone()),
        };
        assert_eq!(signal.status, "filled");
        assert!(signal.fill_details.is_some());
        assert_eq!(signal.fill_details.unwrap()["direction"], "Long");
    }

    #[test]
    fn test_upsert_signal_without_fill_details() {
        let signal = UpsertSignal {
            instrument_id: 2,
            signal_date: NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date"),
            signal_bar_high: Decimal::new(800000, 2),
            signal_bar_low: Decimal::new(795000, 2),
            buy_level: Decimal::new(800200, 2),
            sell_level: Decimal::new(794800, 2),
            status: "pending".into(),
            fill_details: None,
        };
        assert!(signal.fill_details.is_none());
    }

    #[test]
    fn test_signal_status_pending() {
        let row = SignalRow {
            id: Uuid::new_v4(),
            instrument_id: 1,
            signal_date: NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date"),
            signal_bar_high: Decimal::new(1605000, 2),
            signal_bar_low: Decimal::new(1598000, 2),
            buy_level: Decimal::new(1605200, 2),
            sell_level: Decimal::new(1597800, 2),
            status: "pending".into(),
            fill_details: None,
            created_at: Utc::now(),
        };
        assert_eq!(row.status, "pending");
    }

    #[test]
    fn test_signal_status_filled() {
        let row = SignalRow {
            id: Uuid::new_v4(),
            instrument_id: 1,
            signal_date: NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date"),
            signal_bar_high: Decimal::new(1605000, 2),
            signal_bar_low: Decimal::new(1598000, 2),
            buy_level: Decimal::new(1605200, 2),
            sell_level: Decimal::new(1597800, 2),
            status: "filled".into(),
            fill_details: Some(serde_json::json!({"price": 16052})),
            created_at: Utc::now(),
        };
        assert_eq!(row.status, "filled");
        assert!(row.fill_details.is_some());
    }

    #[test]
    fn test_signal_status_expired() {
        let row = SignalRow {
            id: Uuid::new_v4(),
            instrument_id: 1,
            signal_date: NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date"),
            signal_bar_high: Decimal::new(1605000, 2),
            signal_bar_low: Decimal::new(1598000, 2),
            buy_level: Decimal::new(1605200, 2),
            sell_level: Decimal::new(1597800, 2),
            status: "expired".into(),
            fill_details: None,
            created_at: Utc::now(),
        };
        assert_eq!(row.status, "expired");
    }

    #[test]
    fn test_upsert_signal_clone() {
        let signal = UpsertSignal {
            instrument_id: 1,
            signal_date: NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date"),
            signal_bar_high: Decimal::new(1605000, 2),
            signal_bar_low: Decimal::new(1598000, 2),
            buy_level: Decimal::new(1605200, 2),
            sell_level: Decimal::new(1597800, 2),
            status: "pending".into(),
            fill_details: None,
        };
        let cloned = signal.clone();
        assert_eq!(cloned.instrument_id, signal.instrument_id);
        assert_eq!(cloned.status, signal.status);
        assert_eq!(cloned.buy_level, signal.buy_level);
    }
}
