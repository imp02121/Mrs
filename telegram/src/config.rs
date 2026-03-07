//! Bot configuration loaded from environment variables.
//!
//! All settings are read from the process environment. Required variables
//! are `TELEGRAM_BOT_TOKEN` and `DATABASE_URL`. Optional variables have
//! sensible defaults.

use crate::error::BotError;

/// Bot configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Telegram Bot API token from BotFather.
    pub bot_token: String,
    /// Base URL of the sr-engine HTTP API.
    pub engine_api_url: String,
    /// PostgreSQL connection string.
    pub database_url: String,
    /// Optional Valkey/Redis URL for pub/sub.
    pub valkey_url: Option<String>,
    /// Seconds between signal polling cycles.
    pub poll_interval_secs: u64,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// # Required
    /// - `TELEGRAM_BOT_TOKEN`
    /// - `DATABASE_URL`
    ///
    /// # Optional
    /// - `ENGINE_API_URL` (default: `http://localhost:3001`)
    /// - `VALKEY_URL`
    /// - `POLL_INTERVAL_SECS` (default: `30`)
    ///
    /// # Errors
    ///
    /// Returns [`BotError::Config`] if a required variable is missing or
    /// `POLL_INTERVAL_SECS` is not a valid integer.
    pub fn from_env() -> Result<Self, BotError> {
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
            .map_err(|_| BotError::Config("TELEGRAM_BOT_TOKEN is required".into()))?;

        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| BotError::Config("DATABASE_URL is required".into()))?;

        let engine_api_url =
            std::env::var("ENGINE_API_URL").unwrap_or_else(|_| "http://localhost:3001".into());

        let valkey_url = std::env::var("VALKEY_URL").ok();

        let poll_interval_secs = std::env::var("POLL_INTERVAL_SECS")
            .unwrap_or_else(|_| "30".into())
            .parse::<u64>()
            .map_err(|e| BotError::Config(format!("invalid POLL_INTERVAL_SECS: {e}")))?;

        Ok(Self {
            bot_token,
            engine_api_url,
            database_url,
            valkey_url,
            poll_interval_secs,
        })
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[test]
    fn test_config_struct_fields() {
        let config = Config {
            bot_token: "test-token".into(),
            engine_api_url: "http://localhost:3001".into(),
            database_url: "postgres://localhost/test".into(),
            valkey_url: None,
            poll_interval_secs: 30,
        };
        assert_eq!(config.bot_token, "test-token");
        assert_eq!(config.engine_api_url, "http://localhost:3001");
        assert!(config.valkey_url.is_none());
        assert_eq!(config.poll_interval_secs, 30);
    }

    #[test]
    fn test_config_clone() {
        let config = Config {
            bot_token: "tok".into(),
            engine_api_url: "http://engine:3001".into(),
            database_url: "postgres://db/sr".into(),
            valkey_url: Some("redis://localhost".into()),
            poll_interval_secs: 60,
        };
        let cloned = config.clone();
        assert_eq!(cloned.bot_token, "tok");
        assert_eq!(cloned.valkey_url.as_deref(), Some("redis://localhost"));
        assert_eq!(cloned.poll_interval_secs, 60);
    }

    #[test]
    fn test_config_debug() {
        let config = Config {
            bot_token: "secret".into(),
            engine_api_url: "http://localhost:3001".into(),
            database_url: "postgres://localhost/test".into(),
            valkey_url: None,
            poll_interval_secs: 30,
        };
        let debug = format!("{config:?}");
        assert!(debug.contains("Config"));
        assert!(debug.contains("poll_interval_secs"));
    }

    #[test]
    #[serial]
    fn test_config_from_env_missing_token() {
        // SAFETY: test-only, no other threads are accessing these env vars.
        unsafe {
            std::env::remove_var("TELEGRAM_BOT_TOKEN");
        }
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("TELEGRAM_BOT_TOKEN")
        );
    }

    #[test]
    #[serial]
    fn test_config_from_env_missing_database_url() {
        // SAFETY: test-only env var manipulation.
        unsafe {
            std::env::set_var("TELEGRAM_BOT_TOKEN", "test-token-123");
            std::env::remove_var("DATABASE_URL");
        }
        let result = Config::from_env();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("DATABASE_URL"));

        // Cleanup
        unsafe {
            std::env::remove_var("TELEGRAM_BOT_TOKEN");
        }
    }

    #[test]
    #[serial]
    fn test_config_from_env_invalid_poll_interval() {
        // SAFETY: test-only env var manipulation.
        unsafe {
            std::env::set_var("TELEGRAM_BOT_TOKEN", "test-tok");
            std::env::set_var("DATABASE_URL", "postgres://localhost/test");
            std::env::set_var("POLL_INTERVAL_SECS", "not_a_number");
        }
        let result = Config::from_env();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("POLL_INTERVAL_SECS"));

        // Cleanup
        unsafe {
            std::env::remove_var("TELEGRAM_BOT_TOKEN");
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("POLL_INTERVAL_SECS");
        }
    }

    #[test]
    #[serial]
    fn test_config_from_env_custom_poll_interval() {
        // SAFETY: test-only env var manipulation.
        unsafe {
            std::env::set_var("TELEGRAM_BOT_TOKEN", "custom-poll-tok");
            std::env::set_var("DATABASE_URL", "postgres://localhost/custom");
            std::env::set_var("POLL_INTERVAL_SECS", "120");
            std::env::remove_var("ENGINE_API_URL");
            std::env::remove_var("VALKEY_URL");
        }
        let config = Config::from_env().expect("valid config");
        assert_eq!(config.poll_interval_secs, 120);

        // Cleanup
        unsafe {
            std::env::remove_var("TELEGRAM_BOT_TOKEN");
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("POLL_INTERVAL_SECS");
        }
    }

    #[test]
    #[serial]
    fn test_config_from_env_custom_engine_url() {
        // SAFETY: test-only env var manipulation.
        unsafe {
            std::env::set_var("TELEGRAM_BOT_TOKEN", "engine-url-tok");
            std::env::set_var("DATABASE_URL", "postgres://localhost/custom");
            std::env::set_var("ENGINE_API_URL", "http://engine:9999");
            std::env::remove_var("POLL_INTERVAL_SECS");
            std::env::remove_var("VALKEY_URL");
        }
        let config = Config::from_env().expect("valid config");
        assert_eq!(config.engine_api_url, "http://engine:9999");

        // Cleanup
        unsafe {
            std::env::remove_var("TELEGRAM_BOT_TOKEN");
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("ENGINE_API_URL");
        }
    }

    #[test]
    #[serial]
    fn test_config_from_env_valkey_url_set() {
        // SAFETY: test-only env var manipulation.
        unsafe {
            std::env::set_var("TELEGRAM_BOT_TOKEN", "valkey-tok");
            std::env::set_var("DATABASE_URL", "postgres://localhost/valkey");
            std::env::set_var("VALKEY_URL", "redis://cache:6379");
            std::env::remove_var("POLL_INTERVAL_SECS");
            std::env::remove_var("ENGINE_API_URL");
        }
        let config = Config::from_env().expect("valid config");
        assert_eq!(config.valkey_url.as_deref(), Some("redis://cache:6379"));

        // Cleanup
        unsafe {
            std::env::remove_var("TELEGRAM_BOT_TOKEN");
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("VALKEY_URL");
        }
    }

    #[test]
    #[serial]
    fn test_config_from_env_valkey_url_unset() {
        // SAFETY: test-only env var manipulation.
        unsafe {
            std::env::set_var("TELEGRAM_BOT_TOKEN", "no-valkey-tok");
            std::env::set_var("DATABASE_URL", "postgres://localhost/novalkey");
            std::env::remove_var("VALKEY_URL");
            std::env::remove_var("POLL_INTERVAL_SECS");
            std::env::remove_var("ENGINE_API_URL");
        }
        let config = Config::from_env().expect("valid config");
        assert!(config.valkey_url.is_none());

        // Cleanup
        unsafe {
            std::env::remove_var("TELEGRAM_BOT_TOKEN");
            std::env::remove_var("DATABASE_URL");
        }
    }

    #[test]
    #[serial]
    fn test_config_from_env_defaults() {
        // SAFETY: test-only env var manipulation.
        unsafe {
            std::env::set_var("TELEGRAM_BOT_TOKEN", "defaults-tok");
            std::env::set_var("DATABASE_URL", "postgres://localhost/defaults");
            std::env::remove_var("ENGINE_API_URL");
            std::env::remove_var("VALKEY_URL");
            std::env::remove_var("POLL_INTERVAL_SECS");
        }
        let config = Config::from_env().expect("valid config");
        assert_eq!(config.engine_api_url, "http://localhost:3001");
        assert_eq!(config.poll_interval_secs, 30);
        assert!(config.valkey_url.is_none());

        // Cleanup
        unsafe {
            std::env::remove_var("TELEGRAM_BOT_TOKEN");
            std::env::remove_var("DATABASE_URL");
        }
    }
}
