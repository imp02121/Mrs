//! Bot-specific error types.
//!
//! Defines [`BotError`] which covers database, HTTP, configuration,
//! and domain-level errors encountered during bot operation.

/// All errors that can occur within the Telegram bot.
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    /// A database query or connection error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// An HTTP request to the engine API failed.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// An unrecognized instrument name was provided.
    #[error("invalid instrument: {0}")]
    InvalidInstrument(String),

    /// A configuration value is missing or invalid.
    #[error("configuration error: {0}")]
    Config(String),

    /// JSON deserialization failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_instrument_display() {
        let err = BotError::InvalidInstrument("GOLD".into());
        assert_eq!(err.to_string(), "invalid instrument: GOLD");
    }

    #[test]
    fn test_config_error_display() {
        let err = BotError::Config("missing TELEGRAM_BOT_TOKEN".into());
        assert_eq!(
            err.to_string(),
            "configuration error: missing TELEGRAM_BOT_TOKEN"
        );
    }

    #[test]
    fn test_json_error_from() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let bot_err = BotError::from(json_err);
        assert!(bot_err.to_string().contains("json error"));
    }

    #[test]
    fn test_all_variants_produce_non_empty_display() {
        let errors: Vec<BotError> = vec![
            BotError::InvalidInstrument("SP500".into()),
            BotError::Config("missing key".into()),
        ];
        for err in errors {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "error display should not be empty");
            assert!(msg.len() > 5, "error display should be descriptive: {msg}");
        }
    }

    #[test]
    fn test_invalid_instrument_with_empty_name() {
        let err = BotError::InvalidInstrument(String::new());
        assert_eq!(err.to_string(), "invalid instrument: ");
    }

    #[test]
    fn test_config_error_debug_format() {
        let err = BotError::Config("bad config".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("Config"));
        assert!(debug.contains("bad config"));
    }

    #[test]
    fn test_json_error_display_contains_details() {
        let json_err = serde_json::from_str::<Vec<i32>>("{invalid}").unwrap_err();
        let bot_err = BotError::from(json_err);
        let msg = bot_err.to_string();
        assert!(msg.starts_with("json error:"));
        assert!(msg.len() > "json error:".len());
    }

    #[test]
    fn test_database_error_from_sqlx() {
        // Create a sqlx error via a known failure path
        let sqlx_err = sqlx::Error::ColumnNotFound("missing_col".into());
        let bot_err = BotError::from(sqlx_err);
        let msg = bot_err.to_string();
        assert!(msg.contains("database error"));
        assert!(msg.contains("missing_col"));
    }

    #[test]
    fn test_invalid_instrument_with_special_chars() {
        let err = BotError::InvalidInstrument("BTC/USD".into());
        assert_eq!(err.to_string(), "invalid instrument: BTC/USD");
    }

    #[test]
    fn test_config_error_with_unicode() {
        let err = BotError::Config("ung\u{00fc}ltige Konfiguration".into());
        let msg = err.to_string();
        assert!(msg.contains("ung\u{00fc}ltige"));
    }
}
