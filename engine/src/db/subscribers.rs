//! Subscriber CRUD operations.
//!
//! Manages Telegram bot subscribers in the `subscribers` table.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::error::DbError;

/// A row from the `subscribers` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct SubscriberRow {
    /// Auto-generated primary key.
    pub id: i32,
    /// Telegram chat ID.
    pub chat_id: i64,
    /// Telegram username (optional).
    pub username: Option<String>,
    /// Instrument symbols the user is subscribed to.
    pub subscribed_instruments: Vec<String>,
    /// Whether the subscriber is active.
    pub active: bool,
    /// When the subscriber was created.
    pub created_at: DateTime<Utc>,
}

/// Insert a new subscriber (or do nothing on chat_id conflict).
///
/// Returns the subscriber's row ID.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn insert_subscriber(
    pool: &PgPool,
    chat_id: i64,
    username: Option<&str>,
) -> Result<i32, DbError> {
    let row: (i32,) = sqlx::query_as(
        r#"
        INSERT INTO subscribers (chat_id, username)
        VALUES ($1, $2)
        ON CONFLICT (chat_id) DO UPDATE SET
            username = COALESCE(EXCLUDED.username, subscribers.username),
            active = true
        RETURNING id
        "#,
    )
    .bind(chat_id)
    .bind(username)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Get a subscriber by Telegram chat ID.
///
/// Returns `None` if not found.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn get_subscriber(pool: &PgPool, chat_id: i64) -> Result<Option<SubscriberRow>, DbError> {
    let row = sqlx::query_as::<_, SubscriberRow>(
        r#"
        SELECT id, chat_id, username, subscribed_instruments, active, created_at
        FROM subscribers
        WHERE chat_id = $1
        "#,
    )
    .bind(chat_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Update a subscriber's instrument subscriptions.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn update_subscriptions(
    pool: &PgPool,
    chat_id: i64,
    instruments: &[String],
) -> Result<(), DbError> {
    sqlx::query("UPDATE subscribers SET subscribed_instruments = $1 WHERE chat_id = $2")
        .bind(instruments)
        .bind(chat_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// List all active subscribers.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn list_active_subscribers(pool: &PgPool) -> Result<Vec<SubscriberRow>, DbError> {
    let rows = sqlx::query_as::<_, SubscriberRow>(
        r#"
        SELECT id, chat_id, username, subscribed_instruments, active, created_at
        FROM subscribers
        WHERE active = true
        ORDER BY id
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Deactivate a subscriber by chat ID.
///
/// Returns `true` if a row was updated, `false` if the chat_id was not found.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn deactivate_subscriber(pool: &PgPool, chat_id: i64) -> Result<bool, DbError> {
    let result = sqlx::query("UPDATE subscribers SET active = false WHERE chat_id = $1")
        .bind(chat_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscriber_row_construction() {
        let row = SubscriberRow {
            id: 1,
            chat_id: 123456789,
            username: Some("testuser".into()),
            subscribed_instruments: vec!["DAX".into(), "FTSE".into()],
            active: true,
            created_at: Utc::now(),
        };
        assert_eq!(row.chat_id, 123456789);
        assert_eq!(row.subscribed_instruments.len(), 2);
        assert!(row.active);
    }

    #[test]
    fn test_subscriber_row_serde_roundtrip() {
        let row = SubscriberRow {
            id: 2,
            chat_id: 987654321,
            username: None,
            subscribed_instruments: vec!["IXIC".into()],
            active: false,
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: SubscriberRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.chat_id, 987654321);
        assert!(!parsed.active);
    }

    #[test]
    fn test_subscriber_with_empty_subscribed_instruments() {
        let row = SubscriberRow {
            id: 1,
            chat_id: 111222333,
            username: Some("newuser".into()),
            subscribed_instruments: vec![],
            active: true,
            created_at: Utc::now(),
        };
        assert!(row.subscribed_instruments.is_empty());
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: SubscriberRow = serde_json::from_str(&json).expect("deserialize");
        assert!(parsed.subscribed_instruments.is_empty());
    }

    #[test]
    fn test_subscriber_with_multiple_instruments() {
        let row = SubscriberRow {
            id: 3,
            chat_id: 444555666,
            username: Some("poweruser".into()),
            subscribed_instruments: vec!["DAX".into(), "FTSE".into(), "IXIC".into(), "DJI".into()],
            active: true,
            created_at: Utc::now(),
        };
        assert_eq!(row.subscribed_instruments.len(), 4);
        assert!(row.subscribed_instruments.contains(&"DAX".to_string()));
        assert!(row.subscribed_instruments.contains(&"DJI".to_string()));
    }

    #[test]
    fn test_subscriber_username_none() {
        let row = SubscriberRow {
            id: 4,
            chat_id: 777888999,
            username: None,
            subscribed_instruments: vec!["DAX".into()],
            active: true,
            created_at: Utc::now(),
        };
        assert!(row.username.is_none());
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: SubscriberRow = serde_json::from_str(&json).expect("deserialize");
        assert!(parsed.username.is_none());
    }

    #[test]
    fn test_subscriber_username_some() {
        let row = SubscriberRow {
            id: 5,
            chat_id: 111000222,
            username: Some("trader_bob".into()),
            subscribed_instruments: vec!["FTSE".into()],
            active: true,
            created_at: Utc::now(),
        };
        assert_eq!(row.username.as_deref(), Some("trader_bob"));
    }

    #[test]
    fn test_subscriber_clone() {
        let row = SubscriberRow {
            id: 1,
            chat_id: 123456789,
            username: Some("clonetest".into()),
            subscribed_instruments: vec!["DAX".into()],
            active: true,
            created_at: Utc::now(),
        };
        let cloned = row.clone();
        assert_eq!(cloned.chat_id, row.chat_id);
        assert_eq!(cloned.username, row.username);
        assert_eq!(cloned.subscribed_instruments, row.subscribed_instruments);
    }
}
