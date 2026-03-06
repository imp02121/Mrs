//! Strategy configuration CRUD operations.
//!
//! Provides insert, get, list, and delete for the `strategy_configs` table.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::error::DbError;

/// A row from the `strategy_configs` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ConfigRow {
    /// Unique config identifier.
    pub id: Uuid,
    /// Human-readable config name.
    pub name: String,
    /// Strategy parameters stored as JSONB.
    pub params: serde_json::Value,
    /// When this config was created.
    pub created_at: DateTime<Utc>,
}

/// Insert a new strategy configuration.
///
/// Returns the generated UUID.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn insert_config(
    pool: &PgPool,
    name: &str,
    params_json: &serde_json::Value,
) -> Result<Uuid, DbError> {
    let row: (Uuid,) =
        sqlx::query_as("INSERT INTO strategy_configs (name, params) VALUES ($1, $2) RETURNING id")
            .bind(name)
            .bind(params_json)
            .fetch_one(pool)
            .await?;

    Ok(row.0)
}

/// Fetch a strategy configuration by ID.
///
/// # Errors
///
/// Returns [`DbError::NotFound`] if no config matches the ID.
/// Returns [`DbError::Database`] on SQL failure.
pub async fn get_config(pool: &PgPool, id: Uuid) -> Result<ConfigRow, DbError> {
    sqlx::query_as::<_, ConfigRow>(
        "SELECT id, name, params, created_at FROM strategy_configs WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound(format!("config id={id}")))
}

/// List all strategy configurations, newest first.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn list_configs(pool: &PgPool) -> Result<Vec<ConfigRow>, DbError> {
    let rows = sqlx::query_as::<_, ConfigRow>(
        "SELECT id, name, params, created_at FROM strategy_configs ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Delete a strategy configuration by ID.
///
/// Returns `true` if a row was deleted, `false` if it did not exist.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn delete_config(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query("DELETE FROM strategy_configs WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_row_construction() {
        let row = ConfigRow {
            id: Uuid::new_v4(),
            name: "default".into(),
            params: serde_json::json!({"instrument": "DAX"}),
            created_at: Utc::now(),
        };
        assert_eq!(row.name, "default");
    }

    #[test]
    fn test_config_row_serde_roundtrip() {
        let row = ConfigRow {
            id: Uuid::new_v4(),
            name: "test config".into(),
            params: serde_json::json!({"sl_points": 40}),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: ConfigRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.name, "test config");
        assert_eq!(parsed.id, row.id);
    }

    #[test]
    fn test_config_row_with_nested_jsonb_params() {
        let params = serde_json::json!({
            "instrument": "DAX",
            "strategy": {
                "sl_points": 40,
                "tp_multiplier": 2.0,
                "filters": {
                    "min_atr": 10.0,
                    "max_spread": 2.5
                }
            },
            "timeframes": ["15m", "1h", "4h"]
        });
        let row = ConfigRow {
            id: Uuid::new_v4(),
            name: "nested config".into(),
            params: params.clone(),
            created_at: Utc::now(),
        };
        assert_eq!(row.params["strategy"]["sl_points"], 40);
        assert_eq!(row.params["strategy"]["filters"]["min_atr"], 10.0);
        assert!(row.params["timeframes"].is_array());
        assert_eq!(row.params["timeframes"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_config_row_with_array_params() {
        let params = serde_json::json!([1, 2, 3, {"key": "value"}]);
        let row = ConfigRow {
            id: Uuid::new_v4(),
            name: "array config".into(),
            params,
            created_at: Utc::now(),
        };
        assert!(row.params.is_array());
        assert_eq!(row.params.as_array().unwrap().len(), 4);
    }

    #[test]
    fn test_config_row_with_empty_name() {
        let row = ConfigRow {
            id: Uuid::new_v4(),
            name: String::new(),
            params: serde_json::json!({}),
            created_at: Utc::now(),
        };
        assert!(row.name.is_empty());
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: ConfigRow = serde_json::from_str(&json).expect("deserialize");
        assert!(parsed.name.is_empty());
    }

    #[test]
    fn test_config_row_clone() {
        let row = ConfigRow {
            id: Uuid::new_v4(),
            name: "cloneable".into(),
            params: serde_json::json!({"a": 1}),
            created_at: Utc::now(),
        };
        let cloned = row.clone();
        assert_eq!(cloned.id, row.id);
        assert_eq!(cloned.name, row.name);
        assert_eq!(cloned.params, row.params);
    }
}
