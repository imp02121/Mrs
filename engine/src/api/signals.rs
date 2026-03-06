//! Signal endpoint handlers.
//!
//! Provides routes for querying live trading signals — today's signals
//! across all instruments and the latest signal for a specific instrument.

use std::str::FromStr;

use axum::Router;
use axum::extract::{Path, State};
use axum::routing::get;

use crate::db;
use crate::models::Instrument;

use super::error::ApiError;
use super::response::ApiResponse;
use super::state::AppState;

/// Build the signal sub-router.
///
/// Mounts:
/// - `GET /today`
/// - `GET /:instrument/latest`
pub fn signal_routes() -> Router<AppState> {
    Router::new()
        .route("/today", get(today_signals))
        .route("/{instrument}/latest", get(latest_signal))
}

/// `GET /api/signals/today` — get all signals for today across all instruments.
async fn today_signals(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<db::SignalRow>>, ApiError> {
    let signals = db::signals::get_today_signals(&state.db_pool).await?;
    Ok(ApiResponse::new(signals))
}

/// `GET /api/signals/:instrument/latest` — get the latest signal for a specific instrument.
///
/// Parses the instrument from the path segment, resolves the database ID,
/// and returns the most recent signal. Returns 404 if no signal exists.
async fn latest_signal(
    State(state): State<AppState>,
    Path(instrument_str): Path<String>,
) -> Result<ApiResponse<db::SignalRow>, ApiError> {
    let instrument =
        Instrument::from_str(&instrument_str).map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let instrument_id = db::instruments::get_instrument_id(&state.db_pool, instrument).await?;

    let signal = db::signals::get_latest_signal(&state.db_pool, instrument_id)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("no signal found for {}", instrument.ticker()))
        })?;

    Ok(ApiResponse::new(signal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn sample_signal_row() -> db::SignalRow {
        db::SignalRow {
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
        }
    }

    #[test]
    fn test_signal_row_serde_roundtrip() {
        let row = sample_signal_row();
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: db::SignalRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.instrument_id, row.instrument_id);
        assert_eq!(parsed.status, "pending");
    }

    #[test]
    fn test_signal_row_with_fill_details() {
        let mut row = sample_signal_row();
        row.status = "filled".into();
        row.fill_details = Some(serde_json::json!({"price": 16052.00}));
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: db::SignalRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.status, "filled");
        assert!(parsed.fill_details.is_some());
    }

    #[test]
    fn test_instrument_parsing_for_path() {
        let instrument = Instrument::from_str("DAX").expect("valid instrument");
        assert_eq!(instrument, Instrument::Dax);

        let instrument = Instrument::from_str("ftse").expect("valid instrument");
        assert_eq!(instrument, Instrument::Ftse);
    }

    #[test]
    fn test_instrument_parsing_unknown_returns_error() {
        let result = Instrument::from_str("UNKNOWN");
        assert!(result.is_err());
    }

    #[test]
    fn test_instrument_parsing_all_variants() {
        assert_eq!(Instrument::from_str("DAX").unwrap(), Instrument::Dax);
        assert_eq!(Instrument::from_str("FTSE").unwrap(), Instrument::Ftse);
        assert_eq!(Instrument::from_str("NASDAQ").unwrap(), Instrument::Nasdaq);
        assert_eq!(Instrument::from_str("DOW").unwrap(), Instrument::Dow);
    }

    #[test]
    fn test_instrument_parsing_aliases_for_path() {
        assert_eq!(Instrument::from_str("IXIC").unwrap(), Instrument::Nasdaq);
        assert_eq!(Instrument::from_str("DJI").unwrap(), Instrument::Dow);
        assert_eq!(Instrument::from_str("NQ").unwrap(), Instrument::Nasdaq);
        assert_eq!(Instrument::from_str("UKX").unwrap(), Instrument::Ftse);
    }

    #[test]
    fn test_instrument_parsing_case_insensitive_path() {
        assert_eq!(Instrument::from_str("dax").unwrap(), Instrument::Dax);
        assert_eq!(Instrument::from_str("Ftse").unwrap(), Instrument::Ftse);
        assert_eq!(Instrument::from_str("nasdaq").unwrap(), Instrument::Nasdaq);
    }

    #[test]
    fn test_signal_row_serialize_all_fields() {
        let row = sample_signal_row();
        let json = serde_json::to_value(&row).expect("serialize");
        assert!(json.get("id").is_some());
        assert!(json.get("instrument_id").is_some());
        assert!(json.get("signal_date").is_some());
        assert!(json.get("signal_bar_high").is_some());
        assert!(json.get("signal_bar_low").is_some());
        assert!(json.get("buy_level").is_some());
        assert!(json.get("sell_level").is_some());
        assert!(json.get("status").is_some());
        assert!(json.get("fill_details").is_some());
        assert!(json.get("created_at").is_some());
    }

    #[test]
    fn test_signal_row_without_fill_details() {
        let row = sample_signal_row();
        assert!(row.fill_details.is_none());
        let json = serde_json::to_value(&row).expect("serialize");
        assert!(json["fill_details"].is_null());
    }

    #[test]
    fn test_api_response_wraps_signal_row() {
        use super::super::response::ApiResponse;
        let row = sample_signal_row();
        let resp = ApiResponse::new(row);
        let json = serde_json::to_value(&resp).expect("serialize");
        assert!(json.get("data").is_some());
        assert!(json["data"].get("id").is_some());
        assert_eq!(json["data"]["status"], "pending");
    }
}
