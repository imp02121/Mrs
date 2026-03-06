//! Backtest endpoint handlers.
//!
//! Provides routes for running backtests, fetching results, listing trades,
//! comparing configs, and browsing past runs.

use axum::Router;
use axum::extract::{Json, Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::backtest::engine::run_backtest;
use crate::backtest::result::BacktestResult;
use crate::db::{self, BacktestRunRow, InsertBacktestRun};
use crate::models::{Candle, Instrument};
use crate::strategy::config::StrategyConfig;

use super::error::ApiError;
use super::response::{ApiResponse, PaginatedResponse, Pagination, PaginationParams};
use super::state::AppState;

/// Build the backtest sub-router.
pub fn backtest_routes() -> Router<AppState> {
    Router::new()
        .route("/run", post(run_backtest_handler))
        .route("/compare", post(compare_handler))
        .route("/history", get(history_handler))
        .route("/{id}", get(get_backtest_handler))
        .route("/{id}/trades", get(get_trades_handler))
}

// -- Request / Response types --

/// Request body for `POST /api/backtest/run`.
#[derive(Debug, Deserialize)]
pub struct RunBacktestRequest {
    /// The trading instrument (e.g. "DAX", "FTSE").
    pub instrument: String,
    /// Start date for the backtest.
    pub start_date: NaiveDate,
    /// End date for the backtest.
    pub end_date: NaiveDate,
    /// Strategy configuration parameters.
    pub config: StrategyConfig,
}

/// Response for a completed backtest run.
#[derive(Debug, Serialize)]
pub struct BacktestRunResponse {
    /// Database run ID.
    pub run_id: Uuid,
    /// Full backtest result with trades, stats, and equity curve.
    pub result: BacktestResult,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: i64,
}

/// Request body for `POST /api/backtest/compare`.
#[derive(Debug, Deserialize)]
pub struct CompareRequest {
    /// 2-4 backtest configurations to compare.
    pub configs: Vec<RunBacktestRequest>,
}

/// Response for a single comparison result.
#[derive(Debug, Serialize)]
pub struct CompareResultItem {
    /// Run ID for this configuration.
    pub run_id: Uuid,
    /// Backtest result.
    pub result: BacktestResult,
    /// Duration in milliseconds.
    pub duration_ms: i64,
}

/// Summary view of a backtest run for the history listing.
#[derive(Debug, Serialize)]
pub struct BacktestRunSummary {
    /// Unique run identifier.
    pub id: Uuid,
    /// Config ID used.
    pub config_id: Uuid,
    /// Instrument database ID.
    pub instrument_id: i16,
    /// Backtest start date.
    pub start_date: NaiveDate,
    /// Backtest end date.
    pub end_date: NaiveDate,
    /// Number of trades in this run.
    pub total_trades: i32,
    /// Summary statistics.
    pub stats: serde_json::Value,
    /// Wall-clock duration in ms.
    pub duration_ms: i32,
    /// When this run was created.
    pub created_at: DateTime<Utc>,
}

impl From<BacktestRunRow> for BacktestRunSummary {
    fn from(row: BacktestRunRow) -> Self {
        Self {
            id: row.id,
            config_id: row.config_id,
            instrument_id: row.instrument_id,
            start_date: row.start_date,
            end_date: row.end_date,
            total_trades: row.total_trades,
            stats: row.stats,
            duration_ms: row.duration_ms,
            created_at: row.created_at,
        }
    }
}

// -- Handlers --

/// `POST /api/backtest/run` — Run a single backtest.
///
/// Parses the config, fetches candles from the database, runs the backtest
/// engine, saves results to the database, and returns the full result.
async fn run_backtest_handler(
    State(state): State<AppState>,
    Json(req): Json<RunBacktestRequest>,
) -> Result<(StatusCode, ApiResponse<BacktestRunResponse>), ApiError> {
    let (run_id, result, duration_ms) = execute_backtest(&state, req).await?;

    Ok((
        StatusCode::OK,
        ApiResponse::new(BacktestRunResponse {
            run_id,
            result,
            duration_ms,
        }),
    ))
}

/// `GET /api/backtest/:id` — Fetch a backtest result by ID.
///
/// Checks the Valkey cache first via [`CacheReader`], falling back to
/// returning the stored stats from the database.
async fn get_backtest_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    // Try cache first if available
    if let Some(ref cache) = state.cache
        && let Ok(Some(result)) = cache.get_backtest_result(id).await
    {
        return Ok(ApiResponse::new(serde_json::to_value(&result).map_err(
            |e| ApiError::Internal(format!("serialization error: {e}")),
        )?));
    }

    // Fall back to DB
    let row = db::backtests::get_backtest_run(&state.db_pool, id).await?;
    let summary = BacktestRunSummary::from(row);
    Ok(ApiResponse::new(serde_json::to_value(&summary).map_err(
        |e| ApiError::Internal(format!("serialization error: {e}")),
    )?))
}

/// `GET /api/backtest/:id/trades` — Paginated trade list for a run.
async fn get_trades_handler(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<PaginatedResponse<db::TradeRow>, ApiError> {
    let page = params.page();
    let per_page = params.per_page();

    // Verify the run exists
    let _run = db::backtests::get_backtest_run(&state.db_pool, run_id).await?;

    let total = db::trades::count_trades_for_run(&state.db_pool, run_id).await?;
    let trades = db::trades::get_trades_for_run(
        &state.db_pool,
        run_id,
        i64::from(page),
        i64::from(per_page),
    )
    .await?;

    let pagination = Pagination::from_query(page, per_page, total);
    Ok(PaginatedResponse::new(trades, pagination))
}

/// `POST /api/backtest/compare` — Run 2-4 configs and compare results.
async fn compare_handler(
    State(state): State<AppState>,
    Json(req): Json<CompareRequest>,
) -> Result<ApiResponse<Vec<CompareResultItem>>, ApiError> {
    if req.configs.len() < 2 || req.configs.len() > 4 {
        return Err(ApiError::Validation(
            "compare requires 2-4 configurations".into(),
        ));
    }

    let mut results = Vec::with_capacity(req.configs.len());
    for config_req in req.configs {
        let (run_id, result, duration_ms) = execute_backtest(&state, config_req).await?;
        results.push(CompareResultItem {
            run_id,
            result,
            duration_ms,
        });
    }

    Ok(ApiResponse::new(results))
}

/// `GET /api/backtest/history` — List past runs with pagination.
async fn history_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<PaginatedResponse<BacktestRunSummary>, ApiError> {
    let page = params.page();
    let per_page = params.per_page();

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM backtest_runs")
        .fetch_one(&state.db_pool)
        .await
        .map_err(|e| ApiError::Internal(format!("count query failed: {e}")))?;

    let rows =
        db::backtests::list_backtest_runs(&state.db_pool, i64::from(page), i64::from(per_page))
            .await?;

    let summaries: Vec<BacktestRunSummary> = rows.into_iter().map(Into::into).collect();
    let pagination = Pagination::from_query(page, per_page, total.0);
    Ok(PaginatedResponse::new(summaries, pagination))
}

// -- Internal helpers --

/// Execute a backtest: fetch candles, run engine, save to DB, cache result.
///
/// Returns `(run_id, result, duration_ms)`.
async fn execute_backtest(
    state: &AppState,
    req: RunBacktestRequest,
) -> Result<(Uuid, BacktestResult, i64), ApiError> {
    let instrument: Instrument = req
        .instrument
        .parse()
        .map_err(|e: crate::models::ParseInstrumentError| ApiError::BadRequest(e.to_string()))?;

    if req.start_date > req.end_date {
        return Err(ApiError::Validation(
            "start_date must be before or equal to end_date".into(),
        ));
    }

    // Resolve instrument_id from DB
    let instrument_id = db::instruments::get_instrument_id(&state.db_pool, instrument).await?;

    // Fetch candles from DB
    let start_dt: DateTime<Utc> = req
        .start_date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| ApiError::Internal("invalid start datetime".into()))?
        .and_utc();
    let end_dt: DateTime<Utc> = req
        .end_date
        .succ_opt()
        .ok_or_else(|| ApiError::Internal("invalid end datetime".into()))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| ApiError::Internal("invalid end datetime".into()))?
        .and_utc();

    let candle_rows =
        db::candles::get_candles(&state.db_pool, instrument_id, start_dt, end_dt).await?;

    if candle_rows.is_empty() {
        return Err(ApiError::NotFound(format!(
            "no candles found for {} between {} and {}",
            instrument.ticker(),
            req.start_date,
            req.end_date,
        )));
    }

    // Convert CandleRow -> Candle
    let candles: Vec<Candle> = candle_rows
        .into_iter()
        .map(|row| Candle {
            instrument,
            timestamp: row.timestamp,
            open: row.open,
            high: row.high,
            low: row.low,
            close: row.close,
            volume: row.volume,
        })
        .collect();

    // Run backtest
    let mut config = req.config;
    config.instrument = instrument;
    config.date_from = req.start_date;
    config.date_to = req.end_date;

    let timer = std::time::Instant::now();
    let result = run_backtest(&candles, instrument, &config);
    let duration_ms = timer.elapsed().as_millis() as i64;

    // Save config to DB
    let config_json = serde_json::to_value(&config)
        .map_err(|e| ApiError::Internal(format!("config serialization failed: {e}")))?;
    let config_name = format!(
        "{} {} to {}",
        instrument.ticker(),
        req.start_date,
        req.end_date
    );
    let config_id = db::configs::insert_config(&state.db_pool, &config_name, &config_json).await?;

    // Save run to DB
    let stats_json = serde_json::to_value(&result.stats)
        .map_err(|e| ApiError::Internal(format!("stats serialization failed: {e}")))?;
    let insert_run = InsertBacktestRun {
        config_id,
        instrument_id,
        start_date: req.start_date,
        end_date: req.end_date,
        total_trades: result.trade_count() as i32,
        stats: stats_json,
        duration_ms: duration_ms as i32,
    };
    let run_id = db::backtests::insert_backtest_run(&state.db_pool, &insert_run).await?;

    // Save trades to DB
    let trade_inserts: Vec<db::InsertTrade> = result
        .trades
        .iter()
        .map(|t| db::InsertTrade {
            backtest_run_id: run_id,
            instrument_id,
            direction: format!("{:?}", t.direction),
            entry_price: t.entry_price,
            entry_time: t.entry_time,
            exit_price: t.exit_price,
            exit_time: t.exit_time,
            stop_loss: t.stop_loss,
            exit_reason: format!("{:?}", t.exit_reason),
            pnl_points: t.pnl_points,
            pnl_with_adds: t.pnl_with_adds,
            adds: serde_json::to_value(&t.adds).unwrap_or_default(),
            trade_date: t.exit_time.date_naive(),
        })
        .collect();
    db::trades::insert_trades(&state.db_pool, &trade_inserts).await?;

    // Cache result if cache is available
    if let Some(ref cache) = state.cache
        && let Err(e) = cache.set_backtest_result(run_id, &result).await
    {
        tracing::warn!(%run_id, error = %e, "failed to cache backtest result");
    }

    Ok((run_id, result, duration_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_backtest_request_deserialize() {
        let json = serde_json::json!({
            "instrument": "DAX",
            "start_date": "2024-01-01",
            "end_date": "2024-12-31",
            "config": StrategyConfig::default(),
        });
        let req: RunBacktestRequest = serde_json::from_value(json).expect("deserialize");
        assert_eq!(req.instrument, "DAX");
        assert_eq!(
            req.start_date,
            NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date")
        );
    }

    #[test]
    fn test_compare_request_deserialize() {
        let json = serde_json::json!({
            "configs": [
                {
                    "instrument": "DAX",
                    "start_date": "2024-01-01",
                    "end_date": "2024-06-30",
                    "config": StrategyConfig::default(),
                },
                {
                    "instrument": "FTSE",
                    "start_date": "2024-01-01",
                    "end_date": "2024-06-30",
                    "config": StrategyConfig::default(),
                },
            ]
        });
        let req: CompareRequest = serde_json::from_value(json).expect("deserialize");
        assert_eq!(req.configs.len(), 2);
    }

    #[test]
    fn test_backtest_run_summary_from_row() {
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
        let summary = BacktestRunSummary::from(row.clone());
        assert_eq!(summary.id, row.id);
        assert_eq!(summary.total_trades, 150);
    }

    #[test]
    fn test_backtest_run_response_serialize() {
        let result =
            BacktestResult::from_trades(Instrument::Dax, StrategyConfig::default(), Vec::new());
        let resp = BacktestRunResponse {
            run_id: Uuid::new_v4(),
            result,
            duration_ms: 100,
        };
        let json = serde_json::to_value(&resp).expect("serialize");
        assert!(json["run_id"].is_string());
        assert_eq!(json["duration_ms"], 100);
    }

    #[test]
    fn test_pagination_params_defaults() {
        let params = PaginationParams {
            page: None,
            per_page: None,
        };
        assert_eq!(params.page(), 0);
        assert_eq!(params.per_page(), 50);
    }

    #[test]
    fn test_run_backtest_request_with_all_strategy_config_fields() {
        let config = StrategyConfig {
            instrument: Instrument::Ftse,
            signal_bar_index: 3,
            candle_interval_minutes: 5,
            entry_offset_points: rust_decimal::Decimal::new(5, 0),
            allow_both_sides: false,
            sl_mode: crate::strategy::types::StopLossMode::SignalBarExtreme,
            exit_mode: crate::strategy::types::ExitMode::TrailingStop,
            trailing_stop_distance: rust_decimal::Decimal::new(25, 0),
            add_to_winners_enabled: true,
            max_additions: 5,
            session_open: Some(chrono::NaiveTime::from_hms_opt(10, 0, 0).unwrap()),
            session_close: Some(chrono::NaiveTime::from_hms_opt(16, 0, 0).unwrap()),
            signal_expiry_time: Some(chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap()),
            exclude_dates: vec![NaiveDate::from_ymd_opt(2024, 12, 25).unwrap()],
            ..StrategyConfig::default()
        };
        let json = serde_json::json!({
            "instrument": "FTSE",
            "start_date": "2024-01-01",
            "end_date": "2024-06-30",
            "config": config,
        });
        let req: RunBacktestRequest = serde_json::from_value(json).expect("deserialize");
        assert_eq!(req.instrument, "FTSE");
        assert_eq!(req.config.signal_bar_index, 3);
        assert_eq!(req.config.candle_interval_minutes, 5);
        assert!(!req.config.allow_both_sides);
        assert!(req.config.add_to_winners_enabled);
        assert_eq!(req.config.max_additions, 5);
        assert!(req.config.session_open.is_some());
        assert_eq!(req.config.exclude_dates.len(), 1);
    }

    #[test]
    fn test_compare_request_single_config_is_valid_json() {
        // CompareRequest with 1 config deserializes fine (validation is in the handler)
        let json = serde_json::json!({
            "configs": [{
                "instrument": "DAX",
                "start_date": "2024-01-01",
                "end_date": "2024-06-30",
                "config": StrategyConfig::default(),
            }]
        });
        let req: CompareRequest = serde_json::from_value(json).expect("deserialize");
        assert_eq!(req.configs.len(), 1);
    }

    #[test]
    fn test_compare_request_empty_configs() {
        let json = serde_json::json!({"configs": []});
        let req: CompareRequest = serde_json::from_value(json).expect("deserialize");
        assert!(req.configs.is_empty());
    }

    #[test]
    fn test_backtest_run_summary_serde() {
        let summary = BacktestRunSummary {
            id: Uuid::new_v4(),
            config_id: Uuid::new_v4(),
            instrument_id: 1,
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            total_trades: 200,
            stats: serde_json::json!({"win_rate": 0.6, "sharpe": 1.2}),
            duration_ms: 500,
            created_at: Utc::now(),
        };
        let json_str = serde_json::to_string(&summary).expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(value["total_trades"], 200);
        assert_eq!(value["instrument_id"], 1);
        assert_eq!(value["duration_ms"], 500);
        assert!(value["id"].is_string());
        assert!(value["config_id"].is_string());
        assert!(value["start_date"].is_string());
        assert!(value["end_date"].is_string());
        assert!(value["created_at"].is_string());
        assert_eq!(value["stats"]["win_rate"], 0.6);
    }

    #[test]
    fn test_backtest_run_response_has_all_expected_fields() {
        let result =
            BacktestResult::from_trades(Instrument::Dax, StrategyConfig::default(), Vec::new());
        let resp = BacktestRunResponse {
            run_id: Uuid::new_v4(),
            result,
            duration_ms: 250,
        };
        let json = serde_json::to_value(&resp).expect("serialize");
        assert!(json.get("run_id").is_some());
        assert!(json.get("result").is_some());
        assert!(json.get("duration_ms").is_some());
        assert_eq!(json["duration_ms"], 250);
        // result should contain nested fields
        assert!(json["result"].get("instrument").is_some());
        assert!(json["result"].get("config").is_some());
        assert!(json["result"].get("trades").is_some());
        assert!(json["result"].get("equity_curve").is_some());
        assert!(json["result"].get("daily_pnl").is_some());
        assert!(json["result"].get("stats").is_some());
    }

    #[test]
    fn test_compare_result_item_serialize() {
        let result =
            BacktestResult::from_trades(Instrument::Ftse, StrategyConfig::default(), Vec::new());
        let item = CompareResultItem {
            run_id: Uuid::new_v4(),
            result,
            duration_ms: 75,
        };
        let json = serde_json::to_value(&item).expect("serialize");
        assert!(json["run_id"].is_string());
        assert_eq!(json["duration_ms"], 75);
        assert!(json["result"].is_object());
    }

    #[test]
    fn test_backtest_run_summary_from_row_all_fields() {
        let id = Uuid::new_v4();
        let config_id = Uuid::new_v4();
        let now = Utc::now();
        let row = BacktestRunRow {
            id,
            config_id,
            instrument_id: 2,
            start_date: NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 9, 30).unwrap(),
            total_trades: 99,
            stats: serde_json::json!({}),
            duration_ms: 42,
            created_at: now,
        };
        let summary = BacktestRunSummary::from(row);
        assert_eq!(summary.id, id);
        assert_eq!(summary.config_id, config_id);
        assert_eq!(summary.instrument_id, 2);
        assert_eq!(
            summary.start_date,
            NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()
        );
        assert_eq!(
            summary.end_date,
            NaiveDate::from_ymd_opt(2024, 9, 30).unwrap()
        );
        assert_eq!(summary.total_trades, 99);
        assert_eq!(summary.duration_ms, 42);
        assert_eq!(summary.created_at, now);
    }
}
