//! Data endpoint handlers.
//!
//! Provides routes for listing instruments, querying candle data, and
//! triggering data fetches from external providers.

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::db;

use super::error::ApiError;
use super::response::ApiResponse;
use super::state::AppState;

/// Query parameters for the candle endpoint.
#[derive(Debug, Deserialize)]
pub struct CandleQuery {
    /// Instrument ticker or name (e.g. "DAX", "FTSE").
    pub instrument: String,
    /// Start date (inclusive), format `YYYY-MM-DD`.
    pub from: NaiveDate,
    /// End date (exclusive), format `YYYY-MM-DD`.
    pub to: NaiveDate,
}

/// Request body for the data fetch endpoint.
#[derive(Debug, Deserialize)]
pub struct FetchRequest {
    /// Instrument ticker or name.
    pub instrument: String,
    /// Start date.
    pub from: NaiveDate,
    /// End date.
    pub to: NaiveDate,
}

/// Response body for the data fetch placeholder.
#[derive(Debug, Serialize)]
pub struct FetchAccepted {
    /// Status indicator.
    pub status: &'static str,
    /// Human-readable message.
    pub message: &'static str,
}

/// Build the data sub-router.
///
/// Mounts:
/// - `GET /` (instruments)
/// - `GET /candles`
/// - `POST /fetch`
pub fn data_routes() -> Router<AppState> {
    Router::new()
        .route("/instruments", get(list_instruments))
        .route("/candles", get(get_candles))
        .route("/fetch", post(fetch_data))
}

/// `GET /api/data/instruments` — list all instruments from the database.
async fn list_instruments(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<db::InstrumentRow>>, ApiError> {
    let instruments = db::instruments::list_instruments(&state.db_pool).await?;
    Ok(ApiResponse::new(instruments))
}

/// `GET /api/data/candles` — query candle data by instrument and date range.
///
/// Resolves the instrument name to a database ID, converts the date range
/// to UTC timestamps, and fetches matching candles.
async fn get_candles(
    State(state): State<AppState>,
    Query(params): Query<CandleQuery>,
) -> Result<ApiResponse<Vec<db::CandleRow>>, ApiError> {
    if params.from > params.to {
        return Err(ApiError::Validation(format!(
            "from ({}) must be before to ({})",
            params.from, params.to
        )));
    }

    let instrument: crate::models::Instrument = params.instrument.parse().map_err(
        |e: crate::models::instrument::ParseInstrumentError| ApiError::BadRequest(e.to_string()),
    )?;

    let instrument_id = db::instruments::get_instrument_id(&state.db_pool, instrument).await?;

    let start = params
        .from
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| ApiError::Internal("failed to build start timestamp".into()))?
        .and_utc();
    let end = params
        .to
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| ApiError::Internal("failed to build end timestamp".into()))?
        .and_utc();

    let candles = db::candles::get_candles(&state.db_pool, instrument_id, start, end).await?;
    Ok(ApiResponse::new(candles))
}

/// `POST /api/data/fetch` — placeholder for triggering a data fetch.
///
/// Returns 202 Accepted. Actual data fetching will be wired in Phase 8.
async fn fetch_data(
    Json(_body): Json<FetchRequest>,
) -> (axum::http::StatusCode, Json<FetchAccepted>) {
    (
        axum::http::StatusCode::ACCEPTED,
        Json(FetchAccepted {
            status: "accepted",
            message: "Data fetch queued",
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candle_query_deserialize() {
        let json = r#"{"instrument":"DAX","from":"2024-01-01","to":"2024-12-31"}"#;
        let query: CandleQuery = serde_json::from_str(json).expect("deserialize");
        assert_eq!(query.instrument, "DAX");
        assert_eq!(
            query.from,
            NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date")
        );
        assert_eq!(
            query.to,
            NaiveDate::from_ymd_opt(2024, 12, 31).expect("valid date")
        );
    }

    #[test]
    fn test_fetch_request_deserialize() {
        let json = r#"{"instrument":"FTSE","from":"2024-06-01","to":"2024-06-30"}"#;
        let req: FetchRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.instrument, "FTSE");
    }

    #[test]
    fn test_fetch_accepted_serialize() {
        let resp = FetchAccepted {
            status: "accepted",
            message: "Data fetch queued",
        };
        let json = serde_json::to_value(&resp).expect("serialize");
        assert_eq!(json["status"], "accepted");
        assert_eq!(json["message"], "Data fetch queued");
    }

    #[test]
    fn test_fetch_accepted_serde_roundtrip() {
        let resp = FetchAccepted {
            status: "accepted",
            message: "Data fetch queued",
        };
        let json_str = serde_json::to_string(&resp).expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(value["status"], "accepted");
    }

    #[test]
    fn test_candle_query_with_same_dates() {
        let json = r#"{"instrument":"DAX","from":"2024-06-15","to":"2024-06-15"}"#;
        let query: CandleQuery = serde_json::from_str(json).expect("deserialize");
        assert_eq!(query.from, query.to);
    }

    #[test]
    fn test_candle_query_all_fields_present() {
        let json = serde_json::json!({
            "instrument": "NASDAQ",
            "from": "2024-03-01",
            "to": "2024-03-31",
        });
        let query: CandleQuery = serde_json::from_value(json).expect("deserialize");
        assert_eq!(query.instrument, "NASDAQ");
        assert_eq!(query.from, NaiveDate::from_ymd_opt(2024, 3, 1).unwrap());
        assert_eq!(query.to, NaiveDate::from_ymd_opt(2024, 3, 31).unwrap());
    }

    #[test]
    fn test_candle_query_missing_field_fails() {
        let json = r#"{"instrument":"DAX","from":"2024-01-01"}"#;
        let result = serde_json::from_str::<CandleQuery>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_fetch_request_all_fields() {
        let json = serde_json::json!({
            "instrument": "DOW",
            "from": "2024-01-15",
            "to": "2024-02-15",
        });
        let req: FetchRequest = serde_json::from_value(json).expect("deserialize");
        assert_eq!(req.instrument, "DOW");
        assert_eq!(req.from, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        assert_eq!(req.to, NaiveDate::from_ymd_opt(2024, 2, 15).unwrap());
    }

    #[test]
    fn test_fetch_request_missing_field_fails() {
        let json = r#"{"instrument":"FTSE"}"#;
        let result = serde_json::from_str::<FetchRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_fetch_accepted_fields() {
        let resp = FetchAccepted {
            status: "accepted",
            message: "Data fetch queued",
        };
        let json = serde_json::to_value(&resp).expect("serialize");
        assert_eq!(json.as_object().expect("object").len(), 2);
        assert!(json.get("status").is_some());
        assert!(json.get("message").is_some());
    }
}
