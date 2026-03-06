//! Twelve Data API client for fetching historical 15-minute OHLCV candles.
//!
//! Implements [`DataProvider`] using the Twelve Data REST API. Handles
//! pagination (max 5000 data points per request), rate limiting via a
//! [`tokio::sync::Semaphore`], and automatic retry with exponential backoff
//! on transient errors (HTTP 429 and 5xx).
//!
//! # API endpoint
//!
//! ```text
//! GET https://api.twelvedata.com/time_series
//!     ?symbol={ticker}
//!     &interval=15min
//!     &start_date={start}
//!     &end_date={end}
//!     &timezone=UTC
//!     &apikey={key}
//!     &outputsize=5000
//!     &format=JSON
//! ```

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use crate::models::{Candle, DateRange, Instrument};

use super::error::DataError;
use super::provider::DataProvider;

/// Maximum data points the Twelve Data API returns per request.
const MAX_OUTPUT_SIZE: usize = 5000;

/// Maximum days per API request.
///
/// At ~34 candles/day (DAX worst case: 09:00-17:30 = 34 bars) and a
/// 5000-row API limit, this gives 5000/34 ~ 147 days. We use 145
/// as a conservative margin.
const MAX_DAYS_PER_CHUNK: i64 = 145;

/// Maximum number of retry attempts for transient errors.
const MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff (doubles on each retry).
const BASE_RETRY_DELAY: Duration = Duration::from_secs(1);

/// Twelve Data API client.
///
/// Manages rate limiting with a semaphore and an inter-request delay. The
/// free tier allows 8 requests per minute, so the default configuration
/// uses 8 permits with a ~8-second cooldown between requests.
pub struct TwelveDataProvider {
    /// HTTP client (connection-pooled).
    client: reqwest::Client,
    /// API key for authentication.
    api_key: String,
    /// Concurrency limiter.
    semaphore: Arc<Semaphore>,
    /// Minimum delay between consecutive API requests.
    request_delay: Duration,
}

impl TwelveDataProvider {
    /// Create a new provider with the given API key and default rate limits.
    ///
    /// Defaults: 8 concurrent permits, 8-second inter-request delay
    /// (suitable for the free tier: 8 requests/minute).
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            semaphore: Arc::new(Semaphore::new(1)),
            request_delay: Duration::from_secs(8),
        }
    }

    /// Create a provider with custom rate limiting parameters.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Twelve Data API key.
    /// * `max_concurrent` - Maximum number of in-flight API requests.
    /// * `request_delay` - Minimum delay after each request completes
    ///   before the semaphore permit is released.
    pub fn with_rate_limit(
        api_key: impl Into<String>,
        max_concurrent: usize,
        request_delay: Duration,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            request_delay,
        }
    }

    /// Fetch a single page of candle data from the API.
    ///
    /// Acquires a semaphore permit, makes the HTTP request, sleeps for the
    /// configured delay, and releases the permit.
    async fn fetch_page(
        &self,
        instrument: Instrument,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Candle>, DataError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| DataError::Api(format!("semaphore closed: {e}")))?;

        let result = self.fetch_with_retry(instrument, start, end).await;

        // Rate-limit delay before releasing the permit.
        tokio::time::sleep(self.request_delay).await;

        result
    }

    /// Perform the HTTP request with exponential backoff on transient errors.
    async fn fetch_with_retry(
        &self,
        instrument: Instrument,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Candle>, DataError> {
        let mut attempt = 0u32;

        loop {
            match self.do_fetch(instrument, start, end).await {
                Ok(candles) => return Ok(candles),
                Err(e) if is_retryable(&e) && attempt < MAX_RETRIES => {
                    attempt += 1;
                    let delay = BASE_RETRY_DELAY * 2u32.saturating_pow(attempt - 1);
                    warn!(
                        instrument = %instrument,
                        attempt,
                        delay_ms = delay.as_millis() as u64,
                        error = %e,
                        "retrying after transient error"
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Make a single HTTP request to the Twelve Data API and parse the response.
    async fn do_fetch(
        &self,
        instrument: Instrument,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Candle>, DataError> {
        let url = "https://api.twelvedata.com/time_series";

        debug!(
            instrument = %instrument,
            start = %start,
            end = %end,
            "fetching from Twelve Data"
        );

        let resp = self
            .client
            .get(url)
            .query(&[
                ("symbol", instrument.ticker()),
                ("interval", "15min"),
                ("start_date", &start.format("%Y-%m-%d").to_string()),
                ("end_date", &end.format("%Y-%m-%d").to_string()),
                ("timezone", "UTC"),
                ("apikey", &self.api_key),
                ("outputsize", &MAX_OUTPUT_SIZE.to_string()),
                ("format", "JSON"),
            ])
            .send()
            .await?;

        let status = resp.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(DataError::RateLimited {
                retry_after_secs: retry_after,
            });
        }

        if status.is_server_error() {
            let body = resp.text().await.unwrap_or_default();
            return Err(DataError::Api(format!("server error {status}: {body}")));
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(DataError::Api(format!(
                "unexpected status {status}: {body}"
            )));
        }

        let body: TwelveDataResponse = resp.json().await?;

        // Twelve Data returns an error object when the request is semantically invalid.
        if let Some(api_status) = &body.status
            && api_status == "error"
        {
            let msg = body.message.unwrap_or_else(|| "unknown error".into());
            return Err(DataError::Api(msg));
        }

        let values = match body.values {
            Some(v) => v,
            None => return Ok(Vec::new()),
        };

        let mut candles = Vec::with_capacity(values.len());
        for raw in &values {
            match parse_candle_value(raw, instrument) {
                Ok(candle) => candles.push(candle),
                Err(e) => {
                    warn!(
                        instrument = %instrument,
                        datetime = %raw.datetime,
                        error = %e,
                        "skipping invalid candle"
                    );
                }
            }
        }

        // API returns newest first; reverse to chronological order.
        candles.reverse();

        debug!(
            instrument = %instrument,
            count = candles.len(),
            "parsed candles from API response"
        );

        Ok(candles)
    }
}

impl DataProvider for TwelveDataProvider {
    /// Fetch candles for an instrument over a date range.
    ///
    /// Automatically splits long ranges into chunks of [`MAX_DAYS_PER_CHUNK`]
    /// to stay within the API's 5000-row limit per request, then concatenates
    /// and sorts the results.
    async fn fetch_candles(
        &self,
        instrument: Instrument,
        range: DateRange,
    ) -> Result<Vec<Candle>, DataError> {
        let total_days = range.days();

        // If the range fits in a single request, fetch directly.
        if total_days <= MAX_DAYS_PER_CHUNK {
            return self.fetch_page(instrument, range.start, range.end).await;
        }

        // Split into chunks.
        info!(
            instrument = %instrument,
            total_days,
            chunk_days = MAX_DAYS_PER_CHUNK,
            "splitting date range into chunks"
        );

        let mut all_candles = Vec::new();
        let mut chunk_start = range.start;

        while chunk_start <= range.end {
            let chunk_end_candidate = chunk_start + chrono::Duration::days(MAX_DAYS_PER_CHUNK - 1);
            let chunk_end = if chunk_end_candidate > range.end {
                range.end
            } else {
                chunk_end_candidate
            };

            let candles = self.fetch_page(instrument, chunk_start, chunk_end).await?;
            info!(
                instrument = %instrument,
                chunk_start = %chunk_start,
                chunk_end = %chunk_end,
                count = candles.len(),
                "fetched chunk"
            );
            all_candles.extend(candles);

            // Advance to day after chunk_end.
            chunk_start = match chunk_end.succ_opt() {
                Some(d) => d,
                None => break,
            };
        }

        // Sort by timestamp in case chunks overlap.
        all_candles.sort_by_key(|c| c.timestamp);

        Ok(all_candles)
    }
}

/// Determine whether an error is transient and worth retrying.
fn is_retryable(err: &DataError) -> bool {
    match err {
        DataError::RateLimited { .. } | DataError::Http(_) => true,
        DataError::Api(msg) => msg.starts_with("server error"),
        _ => false,
    }
}

// -- Twelve Data JSON response types --

/// Top-level response from the Twelve Data time_series endpoint.
#[derive(Debug, Deserialize)]
struct TwelveDataResponse {
    /// `"ok"` on success, `"error"` on failure.
    status: Option<String>,
    /// Error message when status is `"error"`.
    message: Option<String>,
    /// Array of OHLCV data points (absent on error or empty result).
    values: Option<Vec<TwelveDataValue>>,
}

/// A single OHLCV data point from the Twelve Data API.
///
/// All numeric fields arrive as strings and must be parsed.
#[derive(Debug, Deserialize)]
struct TwelveDataValue {
    /// Timestamp string, e.g. `"2024-01-15 08:15:00"`.
    datetime: String,
    /// Opening price as a string.
    open: String,
    /// High price as a string.
    high: String,
    /// Low price as a string.
    low: String,
    /// Close price as a string.
    close: String,
    /// Volume as a string.
    volume: String,
}

/// Parse a single [`TwelveDataValue`] into a [`Candle`], performing
/// validation on the OHLCV values.
fn parse_candle_value(raw: &TwelveDataValue, instrument: Instrument) -> Result<Candle, DataError> {
    let naive_dt = NaiveDateTime::parse_from_str(&raw.datetime, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| DataError::Validation(format!("invalid datetime '{}': {e}", raw.datetime)))?;
    let timestamp = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);

    let open = Decimal::from_str(&raw.open)
        .map_err(|e| DataError::Validation(format!("invalid open '{}': {e}", raw.open)))?;
    let high = Decimal::from_str(&raw.high)
        .map_err(|e| DataError::Validation(format!("invalid high '{}': {e}", raw.high)))?;
    let low = Decimal::from_str(&raw.low)
        .map_err(|e| DataError::Validation(format!("invalid low '{}': {e}", raw.low)))?;
    let close = Decimal::from_str(&raw.close)
        .map_err(|e| DataError::Validation(format!("invalid close '{}': {e}", raw.close)))?;
    let volume = raw
        .volume
        .parse::<i64>()
        .map_err(|e| DataError::Validation(format!("invalid volume '{}': {e}", raw.volume)))?;

    // Validate OHLCV consistency.
    if high < low {
        return Err(DataError::Validation(format!(
            "high ({high}) < low ({low}) at {}",
            raw.datetime
        )));
    }
    if close < low || close > high {
        return Err(DataError::Validation(format!(
            "close ({close}) outside [low={low}, high={high}] at {}",
            raw.datetime
        )));
    }
    if open < low || open > high {
        return Err(DataError::Validation(format!(
            "open ({open}) outside [low={low}, high={high}] at {}",
            raw.datetime
        )));
    }

    Ok(Candle {
        instrument,
        timestamp,
        open,
        high,
        low,
        close,
        volume,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn make_raw_value(
        datetime: &str,
        open: &str,
        high: &str,
        low: &str,
        close: &str,
        volume: &str,
    ) -> TwelveDataValue {
        TwelveDataValue {
            datetime: datetime.into(),
            open: open.into(),
            high: high.into(),
            low: low.into(),
            close: close.into(),
            volume: volume.into(),
        }
    }

    #[test]
    fn test_parse_candle_value_valid() {
        let raw = make_raw_value(
            "2024-01-15 08:15:00",
            "16000.50",
            "16050.00",
            "15980.25",
            "16030.75",
            "12345",
        );
        let candle = parse_candle_value(&raw, Instrument::Dax).unwrap();
        assert_eq!(candle.instrument, Instrument::Dax);
        assert_eq!(candle.open, d("16000.50"));
        assert_eq!(candle.high, d("16050.00"));
        assert_eq!(candle.low, d("15980.25"));
        assert_eq!(candle.close, d("16030.75"));
        assert_eq!(candle.volume, 12345);
        assert_eq!(candle.timestamp.format("%H:%M").to_string(), "08:15");
    }

    #[test]
    fn test_parse_candle_value_invalid_datetime() {
        let raw = make_raw_value("not-a-date", "100", "110", "90", "105", "1000");
        let err = parse_candle_value(&raw, Instrument::Dax).unwrap_err();
        assert!(err.to_string().contains("invalid datetime"));
    }

    #[test]
    fn test_parse_candle_value_invalid_decimal() {
        let raw = make_raw_value("2024-01-15 08:15:00", "abc", "110", "90", "105", "1000");
        let err = parse_candle_value(&raw, Instrument::Dax).unwrap_err();
        assert!(err.to_string().contains("invalid open"));
    }

    #[test]
    fn test_parse_candle_value_high_less_than_low() {
        let raw = make_raw_value("2024-01-15 08:15:00", "100", "90", "110", "105", "1000");
        let err = parse_candle_value(&raw, Instrument::Dax).unwrap_err();
        assert!(err.to_string().contains("high"));
        assert!(err.to_string().contains("< low"));
    }

    #[test]
    fn test_parse_candle_value_close_outside_range() {
        let raw = make_raw_value("2024-01-15 08:15:00", "100", "110", "90", "120", "1000");
        let err = parse_candle_value(&raw, Instrument::Dax).unwrap_err();
        assert!(err.to_string().contains("close"));
        assert!(err.to_string().contains("outside"));
    }

    #[test]
    fn test_parse_candle_value_open_outside_range() {
        let raw = make_raw_value("2024-01-15 08:15:00", "80", "110", "90", "105", "1000");
        let err = parse_candle_value(&raw, Instrument::Dax).unwrap_err();
        assert!(err.to_string().contains("open"));
        assert!(err.to_string().contains("outside"));
    }

    #[test]
    fn test_parse_candle_value_invalid_volume() {
        let raw = make_raw_value(
            "2024-01-15 08:15:00",
            "100",
            "110",
            "90",
            "105",
            "not-a-number",
        );
        let err = parse_candle_value(&raw, Instrument::Dax).unwrap_err();
        assert!(err.to_string().contains("invalid volume"));
    }

    #[test]
    fn test_parse_candle_value_close_equals_low() {
        // Edge case: close == low should be valid.
        let raw = make_raw_value("2024-01-15 08:15:00", "100", "110", "90", "90", "1000");
        let candle = parse_candle_value(&raw, Instrument::Ftse).unwrap();
        assert_eq!(candle.close, d("90"));
    }

    #[test]
    fn test_parse_candle_value_close_equals_high() {
        // Edge case: close == high should be valid.
        let raw = make_raw_value("2024-01-15 08:15:00", "100", "110", "90", "110", "1000");
        let candle = parse_candle_value(&raw, Instrument::Nasdaq).unwrap();
        assert_eq!(candle.close, d("110"));
    }

    #[test]
    fn test_is_retryable_rate_limited() {
        assert!(is_retryable(&DataError::RateLimited {
            retry_after_secs: 60,
        }));
    }

    #[test]
    fn test_is_retryable_server_error() {
        assert!(is_retryable(&DataError::Api(
            "server error 500: internal".into()
        )));
    }

    #[test]
    fn test_is_not_retryable_validation() {
        assert!(!is_retryable(&DataError::Validation("bad data".into())));
    }

    #[test]
    fn test_is_not_retryable_api_non_server() {
        assert!(!is_retryable(&DataError::Api("invalid API key".into())));
    }

    #[test]
    fn test_max_days_per_chunk_fits_output_size() {
        // DAX worst case: ~34 candles/day (09:00-17:30 = 8.5h * 4 bars/h).
        let candles_per_day: usize = 34;
        assert!(MAX_DAYS_PER_CHUNK as usize * candles_per_day <= MAX_OUTPUT_SIZE);
    }

    #[test]
    fn test_twelve_data_response_deserialize_success() {
        let json = r#"{
            "status": "ok",
            "values": [
                {
                    "datetime": "2024-01-15 08:15:00",
                    "open": "16000.50",
                    "high": "16050.00",
                    "low": "15980.25",
                    "close": "16030.75",
                    "volume": "12345"
                }
            ]
        }"#;
        let resp: TwelveDataResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status.as_deref(), Some("ok"));
        assert_eq!(resp.values.as_ref().map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_twelve_data_response_deserialize_error() {
        let json = r#"{
            "status": "error",
            "message": "Invalid API key"
        }"#;
        let resp: TwelveDataResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status.as_deref(), Some("error"));
        assert_eq!(resp.message.as_deref(), Some("Invalid API key"));
        assert!(resp.values.is_none());
    }

    #[test]
    fn test_twelve_data_provider_new() {
        let provider = TwelveDataProvider::new("test-key");
        assert_eq!(provider.api_key, "test-key");
        assert_eq!(provider.request_delay, Duration::from_secs(8));
    }

    #[test]
    fn test_twelve_data_provider_custom_rate_limit() {
        let provider = TwelveDataProvider::with_rate_limit("key", 4, Duration::from_secs(15));
        assert_eq!(provider.api_key, "key");
        assert_eq!(provider.request_delay, Duration::from_secs(15));
    }

    #[test]
    fn test_parse_valid_response_multi_values() {
        let json = r#"{
            "status": "ok",
            "values": [
                {
                    "datetime": "2024-01-15 09:15:00",
                    "open": "16050.00",
                    "high": "16080.00",
                    "low": "16030.00",
                    "close": "16070.00",
                    "volume": "5000"
                },
                {
                    "datetime": "2024-01-15 08:15:00",
                    "open": "16000.50",
                    "high": "16050.00",
                    "low": "15980.25",
                    "close": "16030.75",
                    "volume": "12345"
                }
            ]
        }"#;
        let resp: TwelveDataResponse = serde_json::from_str(json).unwrap();
        let values = resp.values.unwrap();
        assert_eq!(values.len(), 2);

        let c1 = parse_candle_value(&values[0], Instrument::Dax).unwrap();
        assert_eq!(c1.open, d("16050.00"));

        let c2 = parse_candle_value(&values[1], Instrument::Dax).unwrap();
        assert_eq!(c2.open, d("16000.50"));
    }

    #[test]
    fn test_parse_empty_response() {
        let json = r#"{
            "status": "ok",
            "values": []
        }"#;
        let resp: TwelveDataResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.values.as_ref().map(|v| v.len()), Some(0));
    }

    #[test]
    fn test_parse_response_no_values_field() {
        let json = r#"{
            "status": "ok"
        }"#;
        let resp: TwelveDataResponse = serde_json::from_str(json).unwrap();
        assert!(resp.values.is_none());
    }

    #[test]
    fn test_instrument_ticker_mapping_for_twelve_data() {
        // Verify ticker symbols match what Twelve Data expects.
        assert_eq!(Instrument::Dax.ticker(), "DAX");
        assert_eq!(Instrument::Ftse.ticker(), "FTSE");
        assert_eq!(Instrument::Nasdaq.ticker(), "IXIC");
        assert_eq!(Instrument::Dow.ticker(), "DJI");
    }

    #[test]
    fn test_parse_candle_value_all_instruments() {
        for instrument in Instrument::ALL {
            let raw = make_raw_value("2024-01-15 10:00:00", "100", "110", "90", "105", "500");
            let candle = parse_candle_value(&raw, instrument).unwrap();
            assert_eq!(candle.instrument, instrument);
        }
    }

    #[test]
    fn test_parse_candle_value_high_decimal_precision() {
        let raw = make_raw_value(
            "2024-01-15 08:15:00",
            "16123.456789",
            "16200.123456",
            "16100.000001",
            "16150.999999",
            "99999",
        );
        let candle = parse_candle_value(&raw, Instrument::Dax).unwrap();
        assert_eq!(candle.open, d("16123.456789"));
        assert_eq!(candle.high, d("16200.123456"));
        assert_eq!(candle.low, d("16100.000001"));
        assert_eq!(candle.close, d("16150.999999"));
    }

    #[test]
    fn test_parse_candle_value_flat_candle() {
        // All values equal (open == high == low == close) should be valid.
        let raw = make_raw_value("2024-01-15 08:15:00", "100", "100", "100", "100", "0");
        let candle = parse_candle_value(&raw, Instrument::Ftse).unwrap();
        assert_eq!(candle.open, candle.high);
        assert_eq!(candle.low, candle.close);
    }

    #[test]
    fn test_is_retryable_no_data() {
        assert!(!is_retryable(&DataError::NoData {
            instrument: "DAX".into(),
            start: "2024-01-01".into(),
            end: "2024-01-31".into(),
        }));
    }

    #[test]
    fn test_twelve_data_response_deserialize_with_meta() {
        // Twelve Data responses may include a "meta" field; ensure it doesn't break parsing.
        let json = r#"{
            "meta": {"symbol": "DAX", "interval": "15min"},
            "status": "ok",
            "values": [
                {
                    "datetime": "2024-01-15 08:15:00",
                    "open": "16000.50",
                    "high": "16050.00",
                    "low": "15980.25",
                    "close": "16030.75",
                    "volume": "12345"
                }
            ]
        }"#;
        let resp: TwelveDataResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.values.as_ref().map(|v| v.len()), Some(1));
    }
}
