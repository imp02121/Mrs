//! Router assembly and middleware configuration.
//!
//! [`api_routes`] builds the complete Axum [`Router`] with all endpoint
//! groups nested under `/api` and middleware layers applied. WebSocket
//! routes are mounted at the top level outside `/api`.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use super::state::AppState;
use super::ws::SignalBroadcaster;

/// Build the complete API router with all routes and middleware.
///
/// Mounts sub-routers under `/api`:
/// - `/api/health` — health check
/// - `/api/backtest` — backtest endpoints
/// - `/api/backtest/:id/export` — CSV export
/// - `/api/configs` — config CRUD
/// - `/api/data` — data endpoints
/// - `/api/signals` — signal endpoints
///
/// And WebSocket routes at the top level:
/// - `/ws/signals` — real-time signal streaming
pub fn api_routes(state: AppState) -> Router {
    let broadcaster = Arc::new(SignalBroadcaster::new(state.clone()));

    let backtest =
        super::backtest::backtest_routes().nest("/{id}/export", super::export::export_routes());

    let api = Router::new()
        .route("/health", get(health))
        .nest("/backtest", backtest)
        .nest("/configs", super::configs::config_routes())
        .nest("/data", super::data::data_routes())
        .nest("/signals", super::signals::signal_routes());

    Router::new()
        .nest("/api", api)
        .merge(super::ws::ws_routes(broadcaster))
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(300),
        ))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// `GET /api/health` — returns `{"status": "ok"}`.
async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"status": "ok"}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_duration() {
        let duration = std::time::Duration::from_secs(300);
        assert_eq!(duration.as_secs(), 300);
    }

    #[tokio::test]
    async fn test_health_endpoint_json_format() {
        let resp = health().await;
        let json = resp.0;
        assert_eq!(json["status"], "ok");
        assert_eq!(
            json.as_object().expect("object").len(),
            1,
            "health response should only have 'status' field"
        );
    }
}
