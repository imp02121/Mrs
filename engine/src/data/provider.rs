//! Data provider trait for fetching candle data from external sources.
//!
//! Concrete implementations (e.g. Twelve Data) are defined in separate modules.
//! The trait is kept minimal so that the data fetcher can be provider-agnostic.

use crate::models::{Candle, DateRange, Instrument};

use super::error::DataError;

/// Trait for fetching historical candle data from an external source.
///
/// Implementations handle rate limiting, pagination, and API-specific
/// response parsing. The fetcher module orchestrates calls to this trait
/// and handles storage.
#[allow(async_fn_in_trait)]
pub trait DataProvider {
    /// Fetch candles for an instrument over a date range.
    ///
    /// Returns all available candles within the range, sorted by timestamp.
    /// The implementation may internally paginate or chunk the request.
    ///
    /// # Errors
    ///
    /// Returns [`DataError`] if the fetch fails (network error, rate limit,
    /// invalid response, etc.).
    async fn fetch_candles(
        &self,
        instrument: Instrument,
        range: DateRange,
    ) -> Result<Vec<Candle>, DataError>;
}
