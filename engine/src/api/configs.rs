//! Strategy configuration CRUD endpoint handlers.
//!
//! Provides routes for creating, listing, fetching, and deleting
//! strategy configurations.

use axum::Router;
use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db;
use crate::db::ConfigRow;

use super::error::ApiError;
use super::response::ApiResponse;
use super::state::AppState;

/// Build the config sub-router.
pub fn config_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_config).get(list_configs))
        .route("/{id}", get(get_config).delete(delete_config))
}

// -- Request / Response types --

/// Request body for `POST /api/configs`.
#[derive(Debug, Deserialize)]
pub struct CreateConfigRequest {
    /// Human-readable name for the configuration.
    pub name: String,
    /// Strategy parameters as a JSON object.
    pub params: serde_json::Value,
}

/// Response for a newly created config.
#[derive(Debug, Serialize)]
pub struct CreateConfigResponse {
    /// The generated config ID.
    pub id: Uuid,
}

/// Response for a config listing or single fetch.
#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    /// Unique config identifier.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// Strategy parameters.
    pub params: serde_json::Value,
    /// When this config was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<ConfigRow> for ConfigResponse {
    fn from(row: ConfigRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            params: row.params,
            created_at: row.created_at,
        }
    }
}

// -- Handlers --

/// `POST /api/configs` — Create a new strategy configuration.
async fn create_config(
    State(state): State<AppState>,
    Json(req): Json<CreateConfigRequest>,
) -> Result<(StatusCode, ApiResponse<CreateConfigResponse>), ApiError> {
    if req.name.trim().is_empty() {
        return Err(ApiError::Validation("name must not be empty".into()));
    }

    let id = db::configs::insert_config(&state.db_pool, &req.name, &req.params).await?;

    Ok((
        StatusCode::CREATED,
        ApiResponse::new(CreateConfigResponse { id }),
    ))
}

/// `GET /api/configs` — List all strategy configurations.
async fn list_configs(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<ConfigResponse>>, ApiError> {
    let rows = db::configs::list_configs(&state.db_pool).await?;
    let configs: Vec<ConfigResponse> = rows.into_iter().map(Into::into).collect();
    Ok(ApiResponse::new(configs))
}

/// `GET /api/configs/:id` — Fetch a single configuration by ID.
async fn get_config(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiResponse<ConfigResponse>, ApiError> {
    let row = db::configs::get_config(&state.db_pool, id).await?;
    Ok(ApiResponse::new(ConfigResponse::from(row)))
}

/// `DELETE /api/configs/:id` — Delete a configuration by ID.
async fn delete_config(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = db::configs::delete_config(&state.db_pool, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(format!("config id={id}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_config_request_deserialize() {
        let json = serde_json::json!({
            "name": "test config",
            "params": {"sl_points": 40, "instrument": "DAX"},
        });
        let req: CreateConfigRequest = serde_json::from_value(json).expect("deserialize");
        assert_eq!(req.name, "test config");
        assert_eq!(req.params["sl_points"], 40);
    }

    #[test]
    fn test_create_config_response_serialize() {
        let resp = CreateConfigResponse { id: Uuid::new_v4() };
        let json = serde_json::to_value(&resp).expect("serialize");
        assert!(json["id"].is_string());
    }

    #[test]
    fn test_config_response_from_row() {
        let row = ConfigRow {
            id: Uuid::new_v4(),
            name: "my config".into(),
            params: serde_json::json!({"key": "value"}),
            created_at: chrono::Utc::now(),
        };
        let resp = ConfigResponse::from(row.clone());
        assert_eq!(resp.id, row.id);
        assert_eq!(resp.name, "my config");
    }

    #[test]
    fn test_config_response_serialize() {
        let resp = ConfigResponse {
            id: Uuid::new_v4(),
            name: "test".into(),
            params: serde_json::json!({}),
            created_at: chrono::Utc::now(),
        };
        let json = serde_json::to_value(&resp).expect("serialize");
        assert!(json["id"].is_string());
        assert_eq!(json["name"], "test");
    }

    #[test]
    fn test_create_config_request_roundtrip() {
        let json_str = r#"{"name":"my strategy","params":{"sl_points":40,"instrument":"DAX","exit_mode":"EndOfDay"}}"#;
        let req: CreateConfigRequest = serde_json::from_str(json_str).expect("deserialize");
        assert_eq!(req.name, "my strategy");
        assert_eq!(req.params["sl_points"], 40);
        assert_eq!(req.params["instrument"], "DAX");
        assert_eq!(req.params["exit_mode"], "EndOfDay");
    }

    #[test]
    fn test_create_config_request_with_nested_params() {
        let json = serde_json::json!({
            "name": "complex config",
            "params": {
                "sl_points": 30,
                "nested": {
                    "add_to_winners": true,
                    "levels": [10, 20, 30],
                },
            }
        });
        let req: CreateConfigRequest = serde_json::from_value(json).expect("deserialize");
        assert_eq!(req.name, "complex config");
        assert!(req.params["nested"]["add_to_winners"].as_bool().unwrap());
    }

    #[test]
    fn test_config_response_all_fields_present() {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let resp = ConfigResponse {
            id,
            name: "full config".into(),
            params: serde_json::json!({"key": "val"}),
            created_at: now,
        };
        let json = serde_json::to_value(&resp).expect("serialize");
        assert!(json.get("id").is_some());
        assert!(json.get("name").is_some());
        assert!(json.get("params").is_some());
        assert!(json.get("created_at").is_some());
        assert_eq!(json["name"], "full config");
        assert_eq!(json["params"]["key"], "val");
    }

    #[test]
    fn test_create_config_response_roundtrip() {
        let id = Uuid::new_v4();
        let resp = CreateConfigResponse { id };
        let json_str = serde_json::to_string(&resp).expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(value["id"], id.to_string());
    }

    #[test]
    fn test_config_response_from_row_preserves_all_fields() {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let params = serde_json::json!({"a": 1, "b": "two"});
        let row = ConfigRow {
            id,
            name: "roundtrip".into(),
            params: params.clone(),
            created_at: now,
        };
        let resp = ConfigResponse::from(row);
        assert_eq!(resp.id, id);
        assert_eq!(resp.name, "roundtrip");
        assert_eq!(resp.params, params);
        assert_eq!(resp.created_at, now);
    }
}
