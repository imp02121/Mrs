//! WebSocket endpoint for real-time signal streaming.
//!
//! Clients connect to `/ws/signals` and receive JSON messages whenever
//! today's live signals change. The server polls the database every 10
//! seconds and pushes updates to all connected clients via a broadcast
//! channel.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::db;
use crate::db::SignalRow;

use super::state::AppState;

/// Polling interval for checking signal changes.
const POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Broadcast channel capacity.
const BROADCAST_CAPACITY: usize = 64;

/// Outgoing WebSocket message envelope.
#[derive(Debug, Clone, Serialize)]
pub struct WsMessage {
    /// Message type identifier.
    #[serde(rename = "type")]
    pub msg_type: String,
    /// Payload data (signal rows).
    pub data: Vec<SignalRow>,
}

/// Shared state for WebSocket signal broadcasting.
///
/// Holds a broadcast sender that the polling task writes to and each
/// WebSocket connection subscribes to.
#[derive(Clone)]
pub struct SignalBroadcaster {
    /// Broadcast sender for signal updates.
    tx: broadcast::Sender<String>,
}

impl SignalBroadcaster {
    /// Create a new broadcaster and spawn the polling background task.
    ///
    /// The task queries `db::signals::get_today_signals` every 10 seconds,
    /// compares with the previous snapshot, and broadcasts a JSON message
    /// when the data changes.
    pub fn new(state: AppState) -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let broadcaster = Self { tx: tx.clone() };

        tokio::spawn(async move {
            let mut previous: Option<String> = None;

            loop {
                if let Ok(signals) = db::signals::get_today_signals(&state.db_pool).await {
                    let msg = WsMessage {
                        msg_type: "signal_update".to_owned(),
                        data: signals,
                    };
                    if let Ok(json) = serde_json::to_string(&msg) {
                        let changed = previous.as_ref().is_none_or(|prev| *prev != json);
                        if changed {
                            // Ignore send errors (no receivers)
                            let _ = tx.send(json.clone());
                            previous = Some(json);
                        }
                    }
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        });

        broadcaster
    }
}

/// Build the WebSocket router.
///
/// Mounts `GET /ws/signals` on the main router (outside `/api`).
pub fn ws_routes(broadcaster: Arc<SignalBroadcaster>) -> Router<AppState> {
    Router::new().route(
        "/ws/signals",
        get(move |state, ws| ws_handler(state, ws, broadcaster)),
    )
}

/// WebSocket upgrade handler for `/ws/signals`.
///
/// Upgrades the HTTP connection to WebSocket and spawns a task that
/// sends the initial signals snapshot and then streams updates.
async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    broadcaster: Arc<SignalBroadcaster>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state, broadcaster))
}

/// Handle an individual WebSocket connection.
///
/// Sends the initial signals snapshot, then subscribes to the broadcast
/// channel and forwards updates until the client disconnects.
async fn handle_socket(
    mut socket: WebSocket,
    state: AppState,
    broadcaster: Arc<SignalBroadcaster>,
) {
    // Send initial signals
    if let Ok(signals) = db::signals::get_today_signals(&state.db_pool).await {
        let msg = WsMessage {
            msg_type: "signals".to_owned(),
            data: signals,
        };
        if let Ok(json) = serde_json::to_string(&msg)
            && socket.send(Message::Text(json.into())).await.is_err()
        {
            return;
        }
    }

    // Subscribe to broadcast and forward updates
    let mut rx = broadcaster.tx.subscribe();

    loop {
        match rx.recv().await {
            Ok(json) => {
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "websocket client lagged behind");
            }
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
        }
    }
}

/// Detect whether two signal snapshots differ.
///
/// Compares serialized JSON strings for equality. Returns `true` if
/// the new snapshot is different from the previous one.
#[must_use]
pub fn signals_changed(previous: &str, current: &str) -> bool {
    previous != current
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use rust_decimal::Decimal;

    fn make_test_signal(instrument_id: i16, status: &str) -> SignalRow {
        SignalRow {
            id: uuid::Uuid::new_v4(),
            instrument_id,
            signal_date: NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date"),
            signal_bar_high: Decimal::new(1605000, 2),
            signal_bar_low: Decimal::new(1598000, 2),
            buy_level: Decimal::new(1605200, 2),
            sell_level: Decimal::new(1597800, 2),
            status: status.to_owned(),
            fill_details: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_ws_message_serialization_signals_type() {
        let signals = vec![make_test_signal(1, "pending")];
        let msg = WsMessage {
            msg_type: "signals".to_owned(),
            data: signals,
        };
        let json = serde_json::to_value(&msg).expect("serialize");
        assert_eq!(json["type"], "signals");
        assert!(json["data"].is_array());
        assert_eq!(json["data"].as_array().expect("array").len(), 1);
    }

    #[test]
    fn test_ws_message_serialization_signal_update_type() {
        let signals = vec![
            make_test_signal(1, "pending"),
            make_test_signal(2, "filled"),
        ];
        let msg = WsMessage {
            msg_type: "signal_update".to_owned(),
            data: signals,
        };
        let json = serde_json::to_value(&msg).expect("serialize");
        assert_eq!(json["type"], "signal_update");
        assert_eq!(json["data"].as_array().expect("array").len(), 2);
    }

    #[test]
    fn test_ws_message_empty_signals() {
        let msg = WsMessage {
            msg_type: "signals".to_owned(),
            data: vec![],
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(json.contains("\"type\":\"signals\""));
        assert!(json.contains("\"data\":[]"));
    }

    #[test]
    fn test_signals_changed_detects_difference() {
        let a = r#"{"type":"signals","data":[{"id":"abc"}]}"#;
        let b = r#"{"type":"signals","data":[{"id":"def"}]}"#;
        assert!(signals_changed(a, b));
    }

    #[test]
    fn test_signals_changed_same_content() {
        let a = r#"{"type":"signals","data":[]}"#;
        assert!(!signals_changed(a, a));
    }

    #[test]
    fn test_poll_interval_is_10_seconds() {
        assert_eq!(POLL_INTERVAL, Duration::from_secs(10));
    }

    #[test]
    fn test_broadcast_capacity() {
        assert_eq!(BROADCAST_CAPACITY, 64);
    }

    #[test]
    fn test_ws_message_signal_data_contains_expected_fields() {
        let signal = make_test_signal(1, "pending");
        let msg = WsMessage {
            msg_type: "signals".to_owned(),
            data: vec![signal],
        };
        let json = serde_json::to_value(&msg).expect("serialize");
        let entry = &json["data"][0];
        assert!(entry.get("id").is_some());
        assert!(entry.get("instrument_id").is_some());
        assert!(entry.get("signal_date").is_some());
        assert!(entry.get("signal_bar_high").is_some());
        assert!(entry.get("signal_bar_low").is_some());
        assert!(entry.get("buy_level").is_some());
        assert!(entry.get("sell_level").is_some());
        assert!(entry.get("status").is_some());
        assert!(entry.get("created_at").is_some());
    }

    #[test]
    fn test_ws_message_clone() {
        let msg = WsMessage {
            msg_type: "signals".to_owned(),
            data: vec![],
        };
        let cloned = msg.clone();
        assert_eq!(cloned.msg_type, "signals");
        assert!(cloned.data.is_empty());
    }
}
