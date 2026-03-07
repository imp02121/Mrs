//! Teloxide command handlers.
//!
//! Defines the bot's slash commands and their handler functions.
//! Each command interacts with the subscriber store and/or the
//! engine API to serve user requests.

use std::sync::Arc;

use sqlx::PgPool;
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;

use crate::error::BotError;
use crate::notifications;
use crate::signals;
use crate::store;

/// Valid instrument names for subscription.
const VALID_INSTRUMENTS: [&str; 4] = ["DAX", "FTSE", "NASDAQ", "DOW"];

/// Bot commands available to users.
#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
pub enum Command {
    /// Welcome message and subscriber registration.
    #[command(description = "Start the bot and subscribe")]
    Start,
    /// Show today's trading signals.
    #[command(description = "Show today's signals")]
    Signals,
    /// Subscribe to instruments (comma-separated, e.g. "DAX,FTSE").
    #[command(description = "Subscribe to instruments (e.g. DAX,FTSE)")]
    Subscribe(String),
    /// Unsubscribe from all notifications.
    #[command(description = "Unsubscribe from notifications")]
    Unsubscribe,
    /// Show your current subscription status.
    #[command(description = "Show your subscription status")]
    Status,
}

/// Shared dependencies injected into command handlers.
#[derive(Clone)]
pub struct HandlerDeps {
    /// Database connection pool.
    pub pool: PgPool,
    /// HTTP client for engine API calls.
    pub http_client: reqwest::Client,
    /// Engine API base URL.
    pub engine_api_url: String,
}

/// Validate a list of instrument names.
///
/// Returns the uppercase, deduplicated list of valid instruments and
/// a list of any unrecognized names.
#[must_use]
pub fn validate_instruments(input: &str) -> (Vec<String>, Vec<String>) {
    let mut valid = Vec::new();
    let mut invalid = Vec::new();

    for raw in input.split(',') {
        let name = raw.trim().to_uppercase();
        if name.is_empty() {
            continue;
        }
        if VALID_INSTRUMENTS.contains(&name.as_str()) {
            if !valid.contains(&name) {
                valid.push(name);
            }
        } else {
            invalid.push(name);
        }
    }

    (valid, invalid)
}

/// Top-level command dispatcher.
///
/// Routes each incoming command to its specific handler function.
///
/// # Errors
///
/// Returns `teloxide::RequestError` if any bot message fails to send.
pub async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    deps: Arc<HandlerDeps>,
) -> Result<(), teloxide::RequestError> {
    let chat_id = msg.chat.id.0;

    let response = match cmd {
        Command::Start => handle_start(&deps.pool, chat_id, &msg).await,
        Command::Signals => handle_signals(&deps.engine_api_url, &deps.http_client).await,
        Command::Subscribe(args) => handle_subscribe(&deps.pool, chat_id, &args).await,
        Command::Unsubscribe => handle_unsubscribe(&deps.pool, chat_id).await,
        Command::Status => handle_status(&deps.pool, chat_id).await,
    };

    let text = match response {
        Ok(text) => text,
        Err(e) => {
            tracing::error!(error = %e, "command handler error");
            format!("An error occurred: {e}")
        }
    };

    bot.send_message(msg.chat.id, text).await?;
    Ok(())
}

/// Handle `/start` — register subscriber and show welcome message.
async fn handle_start(pool: &PgPool, chat_id: i64, msg: &Message) -> Result<String, BotError> {
    let username = msg.from.as_ref().and_then(|u| u.username.as_deref());

    store::insert_subscriber(pool, chat_id, username).await?;

    Ok("Welcome to School Run Strategy Bot!\n\n\
         Use /subscribe DAX,FTSE,NASDAQ,DOW to subscribe to instruments.\n\
         Use /signals to see today's signals.\n\
         Use /status to check your subscription.\n\
         Use /unsubscribe to stop notifications."
        .into())
}

/// Handle `/signals` — fetch and display today's signals.
async fn handle_signals(engine_url: &str, client: &reqwest::Client) -> Result<String, BotError> {
    let sigs = signals::poll_signals(engine_url, client).await?;

    if sigs.is_empty() {
        return Ok("No signals for today yet.".into());
    }

    let mut lines = vec!["Today's signals:".to_string(), String::new()];
    for signal in &sigs {
        let name = notifications::instrument_name(signal.instrument_id);
        let high = &signal.signal_bar_high;
        let low = &signal.signal_bar_low;
        let buy = &signal.buy_level;
        let sell = &signal.sell_level;
        lines.push(format!(
            "{name}: High {high} / Low {low} | Buy {buy} / Sell {sell} [{status}]",
            status = signal.status
        ));
    }

    Ok(lines.join("\n"))
}

/// Handle `/subscribe` — parse instruments and update subscriptions.
async fn handle_subscribe(pool: &PgPool, chat_id: i64, args: &str) -> Result<String, BotError> {
    if args.trim().is_empty() {
        return Ok("Please specify instruments to subscribe to.\n\
             Example: /subscribe DAX,FTSE"
            .into());
    }

    let (valid, invalid) = validate_instruments(args);

    if valid.is_empty() {
        return Ok(format!(
            "No valid instruments found. Valid options: {}",
            VALID_INSTRUMENTS.join(", ")
        ));
    }

    store::update_subscriptions(pool, chat_id, &valid).await?;

    let mut response = format!("Subscribed to: {}", valid.join(", "));
    if !invalid.is_empty() {
        response.push_str(&format!("\nUnrecognized (ignored): {}", invalid.join(", ")));
    }

    Ok(response)
}

/// Handle `/unsubscribe` — deactivate subscriber.
async fn handle_unsubscribe(pool: &PgPool, chat_id: i64) -> Result<String, BotError> {
    let deactivated = store::deactivate_subscriber(pool, chat_id).await?;

    if deactivated {
        Ok("You have been unsubscribed. Use /start to re-subscribe.".into())
    } else {
        Ok("You are not currently subscribed.".into())
    }
}

/// Handle `/status` — show subscription details.
async fn handle_status(pool: &PgPool, chat_id: i64) -> Result<String, BotError> {
    let subscriber = store::get_subscriber(pool, chat_id).await?;

    match subscriber {
        Some(sub) if sub.active => {
            let instruments = if sub.subscribed_instruments.is_empty() {
                "none (use /subscribe to add)".into()
            } else {
                sub.subscribed_instruments.join(", ")
            };
            Ok(format!(
                "Status: Active\nInstruments: {instruments}\nSubscribed since: {}",
                sub.created_at.format("%Y-%m-%d")
            ))
        }
        Some(_) => Ok("Status: Inactive\nUse /start to re-subscribe.".into()),
        None => Ok("You are not registered. Use /start to begin.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_instruments_all_valid() {
        let (valid, invalid) = validate_instruments("DAX,FTSE");
        assert_eq!(valid, vec!["DAX", "FTSE"]);
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_validate_instruments_case_insensitive() {
        let (valid, invalid) = validate_instruments("dax, ftse, Nasdaq");
        assert_eq!(valid, vec!["DAX", "FTSE", "NASDAQ"]);
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_validate_instruments_with_invalid() {
        let (valid, invalid) = validate_instruments("DAX,GOLD,FTSE");
        assert_eq!(valid, vec!["DAX", "FTSE"]);
        assert_eq!(invalid, vec!["GOLD"]);
    }

    #[test]
    fn test_validate_instruments_duplicates() {
        let (valid, _) = validate_instruments("DAX,DAX,dax");
        assert_eq!(valid, vec!["DAX"]);
    }

    #[test]
    fn test_validate_instruments_empty() {
        let (valid, invalid) = validate_instruments("");
        assert!(valid.is_empty());
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_validate_instruments_whitespace() {
        let (valid, _) = validate_instruments("  DAX , FTSE  ");
        assert_eq!(valid, vec!["DAX", "FTSE"]);
    }

    #[test]
    fn test_validate_instruments_all_four() {
        let (valid, invalid) = validate_instruments("DAX,FTSE,NASDAQ,DOW");
        assert_eq!(valid.len(), 4);
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_command_descriptions() {
        // BotCommands::descriptions() returns a formatted help string
        let desc = Command::descriptions().to_string();
        assert!(desc.contains("Start the bot"));
        assert!(desc.contains("signals"));
        assert!(desc.contains("Subscribe"));
    }

    #[test]
    fn test_validate_instruments_single_valid() {
        let (valid, invalid) = validate_instruments("DAX");
        assert_eq!(valid, vec!["DAX"]);
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_validate_instruments_single_invalid() {
        let (valid, invalid) = validate_instruments("GOLD");
        assert!(valid.is_empty());
        assert_eq!(invalid, vec!["GOLD"]);
    }

    #[test]
    fn test_validate_instruments_all_invalid() {
        let (valid, invalid) = validate_instruments("GOLD,SP500,BTC,OIL");
        assert!(valid.is_empty());
        assert_eq!(invalid.len(), 4);
        assert!(invalid.contains(&"GOLD".to_string()));
        assert!(invalid.contains(&"SP500".to_string()));
        assert!(invalid.contains(&"BTC".to_string()));
        assert!(invalid.contains(&"OIL".to_string()));
    }

    #[test]
    fn test_validate_instruments_comma_at_start() {
        let (valid, invalid) = validate_instruments(",DAX,FTSE");
        assert_eq!(valid, vec!["DAX", "FTSE"]);
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_validate_instruments_comma_at_end() {
        let (valid, invalid) = validate_instruments("DAX,FTSE,");
        assert_eq!(valid, vec!["DAX", "FTSE"]);
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_validate_instruments_multiple_commas() {
        let (valid, invalid) = validate_instruments("DAX,,FTSE,,,NASDAQ");
        assert_eq!(valid, vec!["DAX", "FTSE", "NASDAQ"]);
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_validate_instruments_just_commas() {
        let (valid, invalid) = validate_instruments(",,,");
        assert!(valid.is_empty());
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_validate_instruments_mixed_valid_and_invalid() {
        let (valid, invalid) = validate_instruments("DAX,GOLD,FTSE,BTC");
        assert_eq!(valid, vec!["DAX", "FTSE"]);
        assert_eq!(invalid, vec!["GOLD", "BTC"]);
    }

    #[test]
    fn test_validate_instruments_aliases_not_accepted() {
        // NQ, DJI, IXIC, UKX are not valid -- only exact names
        let (valid, invalid) = validate_instruments("NQ,DJI,IXIC,UKX");
        assert!(valid.is_empty());
        assert_eq!(invalid.len(), 4);
    }

    #[test]
    fn test_validate_instruments_valid_plus_alias() {
        let (valid, invalid) = validate_instruments("DAX,NQ");
        assert_eq!(valid, vec!["DAX"]);
        assert_eq!(invalid, vec!["NQ"]);
    }

    #[test]
    fn test_validate_instruments_tabs_and_spaces() {
        let (valid, _) = validate_instruments("  DAX  ,\tFTSE\t,  NASDAQ  ");
        assert_eq!(valid, vec!["DAX", "FTSE", "NASDAQ"]);
    }

    #[test]
    fn test_validate_instruments_preserves_order() {
        let (valid, _) = validate_instruments("DOW,NASDAQ,FTSE,DAX");
        assert_eq!(valid, vec!["DOW", "NASDAQ", "FTSE", "DAX"]);
    }

    #[test]
    fn test_validate_instruments_mixed_case_duplicates() {
        let (valid, _) = validate_instruments("dax,DAX,Dax,DaX");
        assert_eq!(valid, vec!["DAX"]);
    }

    #[test]
    fn test_valid_instruments_constant() {
        assert_eq!(VALID_INSTRUMENTS.len(), 4);
        assert!(VALID_INSTRUMENTS.contains(&"DAX"));
        assert!(VALID_INSTRUMENTS.contains(&"FTSE"));
        assert!(VALID_INSTRUMENTS.contains(&"NASDAQ"));
        assert!(VALID_INSTRUMENTS.contains(&"DOW"));
    }
}
