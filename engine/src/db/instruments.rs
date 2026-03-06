//! Instrument database queries.
//!
//! Provides lookups against the `instruments` table seeded by migration 001.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::models::Instrument;

use super::error::DbError;

/// A row from the `instruments` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct InstrumentRow {
    /// Auto-generated primary key.
    pub id: i16,
    /// Ticker symbol (e.g. "DAX").
    pub symbol: String,
    /// Human-readable name.
    pub name: String,
    /// Market open time in local timezone (e.g. "09:00").
    pub open_time_local: String,
    /// Market close time in local timezone (e.g. "17:30").
    pub close_time_local: String,
    /// IANA timezone string.
    pub timezone: String,
    /// Minimum tick size.
    pub tick_size: Decimal,
}

/// Fetch an instrument row by its ticker symbol.
///
/// # Errors
///
/// Returns [`DbError::NotFound`] if no instrument matches the symbol.
/// Returns [`DbError::Database`] on SQL failure.
pub async fn get_instrument_by_symbol(
    pool: &PgPool,
    symbol: &str,
) -> Result<InstrumentRow, DbError> {
    sqlx::query_as::<_, InstrumentRow>(
        "SELECT id, symbol, name, open_time_local, close_time_local, timezone, tick_size FROM instruments WHERE symbol = $1",
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound(format!("instrument symbol={symbol}")))
}

/// List all instruments.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn list_instruments(pool: &PgPool) -> Result<Vec<InstrumentRow>, DbError> {
    let rows = sqlx::query_as::<_, InstrumentRow>(
        "SELECT id, symbol, name, open_time_local, close_time_local, timezone, tick_size FROM instruments ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Get the database ID for a domain [`Instrument`] enum value.
///
/// # Errors
///
/// Returns [`DbError::NotFound`] if the ticker is not in the database.
/// Returns [`DbError::Database`] on SQL failure.
pub async fn get_instrument_id(pool: &PgPool, instrument: Instrument) -> Result<i16, DbError> {
    let row: Option<(i16,)> = sqlx::query_as("SELECT id FROM instruments WHERE symbol = $1")
        .bind(instrument.ticker())
        .fetch_optional(pool)
        .await?;

    row.map(|(id,)| id)
        .ok_or_else(|| DbError::NotFound(format!("instrument {}", instrument.ticker())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instrument_row_construction() {
        let row = InstrumentRow {
            id: 1,
            symbol: "DAX".into(),
            name: "DAX 40".into(),
            open_time_local: "09:00".into(),
            close_time_local: "17:30".into(),
            timezone: "Europe/Berlin".into(),
            tick_size: Decimal::new(50, 2),
        };
        assert_eq!(row.symbol, "DAX");
        assert_eq!(row.id, 1);
    }

    #[test]
    fn test_instrument_row_serde_roundtrip() {
        let row = InstrumentRow {
            id: 2,
            symbol: "FTSE".into(),
            name: "FTSE 100".into(),
            open_time_local: "08:00".into(),
            close_time_local: "16:30".into(),
            timezone: "Europe/London".into(),
            tick_size: Decimal::new(50, 2),
        };
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: InstrumentRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.symbol, "FTSE");
        assert_eq!(parsed.id, 2);
    }

    #[test]
    fn test_dax_seed_data() {
        let row = InstrumentRow {
            id: 1,
            symbol: "DAX".into(),
            name: "DAX 40".into(),
            open_time_local: "09:00".into(),
            close_time_local: "17:30".into(),
            timezone: "Europe/Berlin".into(),
            tick_size: Decimal::new(50, 2),
        };
        assert_eq!(row.symbol, Instrument::Dax.ticker());
        assert_eq!(row.name, Instrument::Dax.name());
        assert_eq!(row.tick_size, Decimal::new(50, 2));
    }

    #[test]
    fn test_ftse_seed_data() {
        let row = InstrumentRow {
            id: 2,
            symbol: "FTSE".into(),
            name: "FTSE 100".into(),
            open_time_local: "08:00".into(),
            close_time_local: "16:30".into(),
            timezone: "Europe/London".into(),
            tick_size: Decimal::new(50, 2),
        };
        assert_eq!(row.symbol, Instrument::Ftse.ticker());
        assert_eq!(row.name, Instrument::Ftse.name());
        assert_eq!(row.tick_size, Decimal::new(50, 2));
    }

    #[test]
    fn test_nasdaq_seed_data() {
        let row = InstrumentRow {
            id: 3,
            symbol: "IXIC".into(),
            name: "Nasdaq Composite".into(),
            open_time_local: "09:30".into(),
            close_time_local: "16:00".into(),
            timezone: "America/New_York".into(),
            tick_size: Decimal::new(25, 2),
        };
        assert_eq!(row.symbol, Instrument::Nasdaq.ticker());
        assert_eq!(row.name, Instrument::Nasdaq.name());
        assert_eq!(row.tick_size, Decimal::new(25, 2));
    }

    #[test]
    fn test_dow_seed_data() {
        let row = InstrumentRow {
            id: 4,
            symbol: "DJI".into(),
            name: "Dow Jones".into(),
            open_time_local: "09:30".into(),
            close_time_local: "16:00".into(),
            timezone: "America/New_York".into(),
            tick_size: Decimal::new(100, 2),
        };
        assert_eq!(row.symbol, Instrument::Dow.ticker());
        assert_eq!(row.name, Instrument::Dow.name());
        assert_eq!(row.tick_size, Decimal::new(100, 2));
    }

    #[test]
    fn test_instrument_row_clone() {
        let row = InstrumentRow {
            id: 1,
            symbol: "DAX".into(),
            name: "DAX 40".into(),
            open_time_local: "09:00".into(),
            close_time_local: "17:30".into(),
            timezone: "Europe/Berlin".into(),
            tick_size: Decimal::new(50, 2),
        };
        let cloned = row.clone();
        assert_eq!(cloned.id, row.id);
        assert_eq!(cloned.symbol, row.symbol);
        assert_eq!(cloned.tick_size, row.tick_size);
    }

    #[test]
    fn test_all_instrument_tickers_match_seed_symbols() {
        let expected_tickers = ["DAX", "FTSE", "IXIC", "DJI"];
        for (instrument, expected) in Instrument::ALL.iter().zip(expected_tickers.iter()) {
            assert_eq!(instrument.ticker(), *expected);
        }
    }
}
