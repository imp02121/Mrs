//! Signal polling and change detection.
//!
//! Periodically fetches today's signals from the sr-engine HTTP API,
//! detects new or changed signals, and dispatches notifications to
//! subscribed Telegram users.

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::PgPool;
use teloxide::prelude::*;

use crate::config::Config;
use crate::notifications;
use crate::store;

/// A single signal as returned by the engine API.
///
/// Mirrors the JSON shape from `GET /api/signals/today`.
#[derive(Debug, Clone, Deserialize)]
pub struct SignalData {
    /// Signal UUID.
    pub id: String,
    /// Foreign key to the instruments table.
    pub instrument_id: i16,
    /// Signal date in `YYYY-MM-DD` format.
    pub signal_date: String,
    /// Signal bar high price.
    pub signal_bar_high: Decimal,
    /// Signal bar low price.
    pub signal_bar_low: Decimal,
    /// Buy stop level.
    pub buy_level: Decimal,
    /// Sell stop level.
    pub sell_level: Decimal,
    /// Signal status (e.g. `"pending"`, `"filled"`, `"expired"`).
    pub status: String,
    /// Optional fill details as arbitrary JSON.
    pub fill_details: Option<serde_json::Value>,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
}

/// Wrapper matching the engine API response envelope.
#[derive(Debug, Deserialize)]
pub struct ApiResponseWrapper {
    /// Payload.
    pub data: Vec<SignalData>,
}

/// Events emitted when a signal changes state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalEvent {
    /// A new signal bar has formed for this instrument.
    NewSignal,
    /// The signal status has changed (e.g. pending -> filled).
    StatusChanged {
        /// Previous status.
        from: String,
        /// New status.
        to: String,
    },
}

/// Tracks previously seen signal states to detect changes.
#[derive(Debug, Default)]
pub struct SignalWatcher {
    /// Maps signal ID to its last-known status.
    seen: HashMap<String, String>,
}

impl SignalWatcher {
    /// Create a new watcher with no prior state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Compare a batch of signals against previously seen state.
    ///
    /// Returns a list of `(SignalData, SignalEvent)` pairs for signals
    /// that are either new or have changed status. Updates internal state.
    pub fn check_for_updates(&mut self, signals: &[SignalData]) -> Vec<(SignalData, SignalEvent)> {
        let mut events = Vec::new();

        for signal in signals {
            match self.seen.get(&signal.id) {
                None => {
                    events.push((signal.clone(), SignalEvent::NewSignal));
                    self.seen.insert(signal.id.clone(), signal.status.clone());
                }
                Some(old_status) if *old_status != signal.status => {
                    events.push((
                        signal.clone(),
                        SignalEvent::StatusChanged {
                            from: old_status.clone(),
                            to: signal.status.clone(),
                        },
                    ));
                    self.seen.insert(signal.id.clone(), signal.status.clone());
                }
                Some(_) => {
                    // No change
                }
            }
        }

        events
    }
}

/// Fetch today's signals from the engine API.
///
/// # Errors
///
/// Returns an error if the HTTP request fails or the response is not valid JSON.
pub async fn poll_signals(
    engine_url: &str,
    client: &reqwest::Client,
) -> Result<Vec<SignalData>, crate::error::BotError> {
    let url = format!("{engine_url}/api/signals/today");
    let resp = client.get(&url).send().await?;
    let wrapper: ApiResponseWrapper = resp.json().await?;
    Ok(wrapper.data)
}

/// Run the signal polling loop.
///
/// Spawns an infinite loop that polls the engine API at `config.poll_interval_secs`
/// intervals, detects changes, and sends notifications to subscribed users.
///
/// This function is intended to be run via `tokio::spawn`.
pub async fn run_signal_loop(config: Config, pool: PgPool, bot: Bot) {
    let client = reqwest::Client::new();
    let mut watcher = SignalWatcher::new();
    let interval = std::time::Duration::from_secs(config.poll_interval_secs);

    tracing::info!(
        poll_interval_secs = config.poll_interval_secs,
        engine_url = %config.engine_api_url,
        "starting signal polling loop"
    );

    loop {
        match poll_signals(&config.engine_api_url, &client).await {
            Ok(signals) => {
                let events = watcher.check_for_updates(&signals);
                for (signal, event) in events {
                    let instrument = notifications::instrument_name(signal.instrument_id);
                    tracing::info!(%instrument, ?event, "signal event detected");

                    let message = match &event {
                        SignalEvent::NewSignal => notifications::format_signal_bar_formed(&signal),
                        SignalEvent::StatusChanged { to, .. } if to == "filled" => {
                            // Determine direction from fill_details if available
                            let direction = signal
                                .fill_details
                                .as_ref()
                                .and_then(|d| d.get("direction"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("LONG");
                            let entry = signal
                                .fill_details
                                .as_ref()
                                .and_then(|d| d.get("price"))
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse::<Decimal>().ok())
                                .unwrap_or(signal.buy_level);
                            let stop_loss = signal.signal_bar_low;
                            notifications::format_order_triggered(
                                &signal, direction, &entry, &stop_loss,
                            )
                        }
                        SignalEvent::StatusChanged { to, .. } => {
                            format!("{} signal status changed to: {}", instrument, to)
                        }
                    };

                    // Send to subscribers of this instrument
                    match store::get_subscribers_for_instrument(&pool, instrument).await {
                        Ok(subscribers) => {
                            for sub in subscribers {
                                if let Err(e) =
                                    notifications::send_notification(&bot, sub.chat_id, &message)
                                        .await
                                {
                                    tracing::warn!(
                                        chat_id = sub.chat_id,
                                        error = %e,
                                        "failed to send notification"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "failed to query subscribers");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to poll signals");
            }
        }

        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_signal(id: &str, instrument_id: i16, status: &str) -> SignalData {
        SignalData {
            id: id.into(),
            instrument_id,
            signal_date: "2024-06-15".into(),
            signal_bar_high: dec!(22448.00),
            signal_bar_low: dec!(22390.00),
            buy_level: dec!(22450.00),
            sell_level: dec!(22388.00),
            status: status.into(),
            fill_details: None,
            created_at: String::new(),
        }
    }

    #[test]
    fn test_signal_watcher_detects_new_signal() {
        let mut watcher = SignalWatcher::new();
        let signals = vec![make_signal("s1", 1, "pending")];
        let events = watcher.check_for_updates(&signals);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, SignalEvent::NewSignal);
    }

    #[test]
    fn test_signal_watcher_no_change() {
        let mut watcher = SignalWatcher::new();
        let signals = vec![make_signal("s1", 1, "pending")];
        watcher.check_for_updates(&signals);

        // Same signals again
        let events = watcher.check_for_updates(&signals);
        assert!(events.is_empty());
    }

    #[test]
    fn test_signal_watcher_detects_status_change() {
        let mut watcher = SignalWatcher::new();
        let signals = vec![make_signal("s1", 1, "pending")];
        watcher.check_for_updates(&signals);

        let updated = vec![make_signal("s1", 1, "filled")];
        let events = watcher.check_for_updates(&updated);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].1,
            SignalEvent::StatusChanged {
                from: "pending".into(),
                to: "filled".into()
            }
        );
    }

    #[test]
    fn test_signal_watcher_multiple_signals() {
        let mut watcher = SignalWatcher::new();
        let signals = vec![
            make_signal("s1", 1, "pending"),
            make_signal("s2", 2, "pending"),
        ];
        let events = watcher.check_for_updates(&signals);
        assert_eq!(events.len(), 2);

        // Only s2 changes
        let updated = vec![
            make_signal("s1", 1, "pending"),
            make_signal("s2", 2, "expired"),
        ];
        let events = watcher.check_for_updates(&updated);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0.id, "s2");
    }

    #[test]
    fn test_signal_data_deserialize() {
        let json = r#"{
            "id": "abc-123",
            "instrument_id": 1,
            "signal_date": "2024-06-15",
            "signal_bar_high": "22448.00",
            "signal_bar_low": "22390.00",
            "buy_level": "22450.00",
            "sell_level": "22388.00",
            "status": "pending",
            "fill_details": null,
            "created_at": "2024-06-15T08:30:00Z"
        }"#;
        let signal: SignalData = serde_json::from_str(json).expect("deserialize signal");
        assert_eq!(signal.instrument_id, 1);
        assert_eq!(signal.status, "pending");
        assert_eq!(signal.signal_bar_high, dec!(22448.00));
    }

    #[test]
    fn test_api_response_wrapper_deserialize() {
        let json = r#"{"data": [
            {
                "id": "abc",
                "instrument_id": 1,
                "signal_date": "2024-06-15",
                "signal_bar_high": "16050.00",
                "signal_bar_low": "15980.00",
                "buy_level": "16052.00",
                "sell_level": "15978.00",
                "status": "pending",
                "fill_details": null,
                "created_at": "2024-06-15T08:30:00Z"
            }
        ]}"#;
        let wrapper: ApiResponseWrapper = serde_json::from_str(json).expect("deserialize wrapper");
        assert_eq!(wrapper.data.len(), 1);
        assert_eq!(wrapper.data[0].instrument_id, 1);
    }

    #[test]
    fn test_signal_event_equality() {
        assert_eq!(SignalEvent::NewSignal, SignalEvent::NewSignal);
        assert_ne!(
            SignalEvent::NewSignal,
            SignalEvent::StatusChanged {
                from: "a".into(),
                to: "b".into()
            }
        );
    }

    #[test]
    fn test_signal_watcher_default() {
        let watcher = SignalWatcher::default();
        assert!(watcher.seen.is_empty());
    }

    #[test]
    fn test_signal_watcher_multiple_signals_changing_at_once() {
        let mut watcher = SignalWatcher::new();
        let initial = vec![
            make_signal("s1", 1, "pending"),
            make_signal("s2", 2, "pending"),
            make_signal("s3", 3, "pending"),
        ];
        watcher.check_for_updates(&initial);

        // All three change status simultaneously
        let updated = vec![
            make_signal("s1", 1, "filled"),
            make_signal("s2", 2, "expired"),
            make_signal("s3", 3, "filled"),
        ];
        let events = watcher.check_for_updates(&updated);
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0].1,
            SignalEvent::StatusChanged {
                from: "pending".into(),
                to: "filled".into()
            }
        );
        assert_eq!(
            events[1].1,
            SignalEvent::StatusChanged {
                from: "pending".into(),
                to: "expired".into()
            }
        );
        assert_eq!(
            events[2].1,
            SignalEvent::StatusChanged {
                from: "pending".into(),
                to: "filled".into()
            }
        );
    }

    #[test]
    fn test_signal_watcher_signal_disappears() {
        let mut watcher = SignalWatcher::new();
        let signals = vec![
            make_signal("s1", 1, "pending"),
            make_signal("s2", 2, "pending"),
        ];
        watcher.check_for_updates(&signals);

        // s1 disappears from the response -- watcher should not emit events
        // for missing signals (they just stay in the seen map)
        let remaining = vec![make_signal("s2", 2, "pending")];
        let events = watcher.check_for_updates(&remaining);
        assert!(events.is_empty());
    }

    #[test]
    fn test_signal_watcher_same_signal_twice_idempotent() {
        let mut watcher = SignalWatcher::new();
        let signals = vec![
            make_signal("s1", 1, "pending"),
            make_signal("s1", 1, "pending"), // duplicate in same batch
        ];
        let events = watcher.check_for_updates(&signals);
        // First occurrence is new, second is a no-change
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, SignalEvent::NewSignal);
    }

    #[test]
    fn test_signal_watcher_empty_then_signals() {
        let mut watcher = SignalWatcher::new();

        // First poll: no signals
        let events = watcher.check_for_updates(&[]);
        assert!(events.is_empty());

        // Second poll: signals appear
        let signals = vec![make_signal("s1", 1, "pending")];
        let events = watcher.check_for_updates(&signals);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, SignalEvent::NewSignal);
    }

    #[test]
    fn test_signal_watcher_status_change_chain() {
        let mut watcher = SignalWatcher::new();

        // pending -> filled -> closed (multi-step transitions)
        let signals = vec![make_signal("s1", 1, "pending")];
        let events = watcher.check_for_updates(&signals);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, SignalEvent::NewSignal);

        let signals = vec![make_signal("s1", 1, "filled")];
        let events = watcher.check_for_updates(&signals);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].1,
            SignalEvent::StatusChanged {
                from: "pending".into(),
                to: "filled".into()
            }
        );

        let signals = vec![make_signal("s1", 1, "closed")];
        let events = watcher.check_for_updates(&signals);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].1,
            SignalEvent::StatusChanged {
                from: "filled".into(),
                to: "closed".into()
            }
        );
    }

    #[test]
    fn test_signal_watcher_new_signal_after_seen() {
        let mut watcher = SignalWatcher::new();
        let signals = vec![make_signal("s1", 1, "pending")];
        watcher.check_for_updates(&signals);

        // A brand new signal alongside the existing one
        let signals = vec![
            make_signal("s1", 1, "pending"),
            make_signal("s2", 2, "pending"),
        ];
        let events = watcher.check_for_updates(&signals);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0.id, "s2");
        assert_eq!(events[0].1, SignalEvent::NewSignal);
    }

    #[test]
    fn test_signal_data_deserialize_with_fill_details() {
        let json = r#"{
            "id": "fill-123",
            "instrument_id": 1,
            "signal_date": "2024-06-15",
            "signal_bar_high": "22448.00",
            "signal_bar_low": "22390.00",
            "buy_level": "22450.00",
            "sell_level": "22388.00",
            "status": "filled",
            "fill_details": {"direction": "LONG", "price": "22450.00"},
            "created_at": "2024-06-15T09:30:00Z"
        }"#;
        let signal: SignalData = serde_json::from_str(json).expect("deserialize signal");
        assert_eq!(signal.status, "filled");
        assert!(signal.fill_details.is_some());
        let details = signal.fill_details.unwrap();
        assert_eq!(details["direction"], "LONG");
        assert_eq!(details["price"], "22450.00");
    }

    #[test]
    fn test_api_response_wrapper_empty_data() {
        let json = r#"{"data": []}"#;
        let wrapper: ApiResponseWrapper = serde_json::from_str(json).expect("deserialize wrapper");
        assert!(wrapper.data.is_empty());
    }

    #[test]
    fn test_api_response_wrapper_multiple_signals() {
        let json = r#"{"data": [
            {
                "id": "a1",
                "instrument_id": 1,
                "signal_date": "2024-06-15",
                "signal_bar_high": "22000.00",
                "signal_bar_low": "21950.00",
                "buy_level": "22002.00",
                "sell_level": "21948.00",
                "status": "pending",
                "fill_details": null,
                "created_at": "2024-06-15T08:30:00Z"
            },
            {
                "id": "a2",
                "instrument_id": 2,
                "signal_date": "2024-06-15",
                "signal_bar_high": "7650.00",
                "signal_bar_low": "7620.00",
                "buy_level": "7652.00",
                "sell_level": "7618.00",
                "status": "filled",
                "fill_details": {"direction": "SHORT", "price": "7618.00"},
                "created_at": "2024-06-15T08:30:00Z"
            }
        ]}"#;
        let wrapper: ApiResponseWrapper = serde_json::from_str(json).expect("deserialize wrapper");
        assert_eq!(wrapper.data.len(), 2);
        assert_eq!(wrapper.data[0].instrument_id, 1);
        assert_eq!(wrapper.data[1].instrument_id, 2);
        assert_eq!(wrapper.data[1].status, "filled");
    }

    #[test]
    fn test_signal_event_debug() {
        let event = SignalEvent::StatusChanged {
            from: "pending".into(),
            to: "filled".into(),
        };
        let debug = format!("{event:?}");
        assert!(debug.contains("StatusChanged"));
        assert!(debug.contains("pending"));
        assert!(debug.contains("filled"));
    }

    #[test]
    fn test_signal_event_clone() {
        let event = SignalEvent::StatusChanged {
            from: "pending".into(),
            to: "expired".into(),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn test_signal_data_clone() {
        let signal = make_signal("clone-test", 3, "pending");
        let cloned = signal.clone();
        assert_eq!(signal.id, cloned.id);
        assert_eq!(signal.instrument_id, cloned.instrument_id);
        assert_eq!(signal.status, cloned.status);
        assert_eq!(signal.signal_bar_high, cloned.signal_bar_high);
    }
}
