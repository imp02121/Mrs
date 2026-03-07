//! Subscriber persistence layer.
//!
//! CRUD operations for the `subscribers` table, providing chat-ID-based
//! lookups, instrument subscriptions, and activation/deactivation.

use sqlx::PgPool;

use crate::error::BotError;

/// A row from the `subscribers` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SubscriberRow {
    /// Auto-incremented primary key.
    pub id: i32,
    /// Telegram chat ID (unique per subscriber).
    pub chat_id: i64,
    /// Telegram username, if available.
    pub username: Option<String>,
    /// List of subscribed instrument names (e.g. `["DAX", "FTSE"]`).
    pub subscribed_instruments: Vec<String>,
    /// Whether the subscriber is currently active.
    pub active: bool,
    /// When the subscriber was first created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Insert a new subscriber or re-activate an existing one.
///
/// If the chat ID already exists, the subscriber is re-activated and the
/// username is updated. Returns the subscriber's database ID.
///
/// # Errors
///
/// Returns [`BotError::Database`] on connection or query failure.
pub async fn insert_subscriber(
    pool: &PgPool,
    chat_id: i64,
    username: Option<&str>,
) -> Result<i32, BotError> {
    let row = sqlx::query_scalar::<_, i32>(
        "INSERT INTO subscribers (chat_id, username, active)
         VALUES ($1, $2, true)
         ON CONFLICT (chat_id) DO UPDATE SET active = true, username = $2
         RETURNING id",
    )
    .bind(chat_id)
    .bind(username)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Fetch a subscriber by chat ID.
///
/// Returns `None` if no subscriber with this chat ID exists.
///
/// # Errors
///
/// Returns [`BotError::Database`] on connection or query failure.
pub async fn get_subscriber(
    pool: &PgPool,
    chat_id: i64,
) -> Result<Option<SubscriberRow>, BotError> {
    let row = sqlx::query_as::<_, SubscriberRow>(
        "SELECT id, chat_id, username, subscribed_instruments, active, created_at
         FROM subscribers WHERE chat_id = $1",
    )
    .bind(chat_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Update the list of subscribed instruments for a given chat ID.
///
/// # Errors
///
/// Returns [`BotError::Database`] on connection or query failure.
pub async fn update_subscriptions(
    pool: &PgPool,
    chat_id: i64,
    instruments: &[String],
) -> Result<(), BotError> {
    sqlx::query("UPDATE subscribers SET subscribed_instruments = $2 WHERE chat_id = $1")
        .bind(chat_id)
        .bind(instruments)
        .execute(pool)
        .await?;

    Ok(())
}

/// List all active subscribers.
///
/// # Errors
///
/// Returns [`BotError::Database`] on connection or query failure.
pub async fn list_active_subscribers(pool: &PgPool) -> Result<Vec<SubscriberRow>, BotError> {
    let rows = sqlx::query_as::<_, SubscriberRow>(
        "SELECT id, chat_id, username, subscribed_instruments, active, created_at
         FROM subscribers WHERE active = true",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// List active subscribers for a specific instrument.
///
/// Matches subscribers whose `subscribed_instruments` array contains the
/// given instrument name.
///
/// # Errors
///
/// Returns [`BotError::Database`] on connection or query failure.
pub async fn get_subscribers_for_instrument(
    pool: &PgPool,
    instrument: &str,
) -> Result<Vec<SubscriberRow>, BotError> {
    let rows = sqlx::query_as::<_, SubscriberRow>(
        "SELECT id, chat_id, username, subscribed_instruments, active, created_at
         FROM subscribers WHERE active = true AND $1 = ANY(subscribed_instruments)",
    )
    .bind(instrument)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Deactivate a subscriber by chat ID.
///
/// Returns `true` if a row was updated, `false` if no subscriber was found.
///
/// # Errors
///
/// Returns [`BotError::Database`] on connection or query failure.
pub async fn deactivate_subscriber(pool: &PgPool, chat_id: i64) -> Result<bool, BotError> {
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
    fn test_subscriber_row_debug() {
        // SubscriberRow derives Debug
        let row = SubscriberRow {
            id: 1,
            chat_id: 12345,
            username: Some("testuser".into()),
            subscribed_instruments: vec!["DAX".into(), "FTSE".into()],
            active: true,
            created_at: chrono::Utc::now(),
        };
        let debug = format!("{row:?}");
        assert!(debug.contains("testuser"));
        assert!(debug.contains("DAX"));
    }

    #[test]
    fn test_subscriber_row_clone() {
        let row = SubscriberRow {
            id: 2,
            chat_id: 67890,
            username: None,
            subscribed_instruments: vec![],
            active: false,
            created_at: chrono::Utc::now(),
        };
        let cloned = row.clone();
        assert_eq!(cloned.chat_id, 67890);
        assert!(!cloned.active);
        assert!(cloned.username.is_none());
    }

    #[test]
    fn test_subscriber_row_empty_instruments() {
        let row = SubscriberRow {
            id: 3,
            chat_id: 11111,
            username: None,
            subscribed_instruments: vec![],
            active: true,
            created_at: chrono::Utc::now(),
        };
        assert!(row.subscribed_instruments.is_empty());
    }

    #[test]
    fn test_subscriber_row_all_instruments() {
        let row = SubscriberRow {
            id: 4,
            chat_id: 99999,
            username: Some("full_sub".into()),
            subscribed_instruments: vec![
                "DAX".into(),
                "FTSE".into(),
                "NASDAQ".into(),
                "DOW".into(),
            ],
            active: true,
            created_at: chrono::Utc::now(),
        };
        assert_eq!(row.subscribed_instruments.len(), 4);
        assert!(row.subscribed_instruments.contains(&"DAX".to_string()));
        assert!(row.subscribed_instruments.contains(&"DOW".to_string()));
    }

    #[test]
    fn test_subscriber_row_field_types() {
        let now = chrono::Utc::now();
        let row = SubscriberRow {
            id: 42,
            chat_id: i64::MAX,
            username: Some("max_chat_id_user".into()),
            subscribed_instruments: vec!["DAX".into()],
            active: false,
            created_at: now,
        };
        // Verify i64 chat_id supports large Telegram chat IDs
        assert_eq!(row.chat_id, i64::MAX);
        assert_eq!(row.id, 42);
        assert!(!row.active);
        assert_eq!(row.created_at, now);
    }

    #[test]
    fn test_subscriber_row_negative_chat_id() {
        // Group chats in Telegram have negative chat IDs
        let row = SubscriberRow {
            id: 5,
            chat_id: -1001234567890,
            username: None,
            subscribed_instruments: vec!["FTSE".into()],
            active: true,
            created_at: chrono::Utc::now(),
        };
        assert!(row.chat_id < 0);
        assert_eq!(row.chat_id, -1001234567890);
    }

    #[test]
    fn test_subscriber_row_clone_preserves_all_fields() {
        let now = chrono::Utc::now();
        let row = SubscriberRow {
            id: 10,
            chat_id: 55555,
            username: Some("clone_test".into()),
            subscribed_instruments: vec!["NASDAQ".into(), "DOW".into()],
            active: true,
            created_at: now,
        };
        let cloned = row.clone();
        assert_eq!(row.id, cloned.id);
        assert_eq!(row.chat_id, cloned.chat_id);
        assert_eq!(row.username, cloned.username);
        assert_eq!(row.subscribed_instruments, cloned.subscribed_instruments);
        assert_eq!(row.active, cloned.active);
        assert_eq!(row.created_at, cloned.created_at);
    }
}
