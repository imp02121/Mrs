# Data Pipeline Architecture

This document describes the internal architecture of the School Run data pipeline: how candle data flows from the Twelve Data API into local Parquet files and PostgreSQL.

## Overview

```
                    +---------------------+
                    |   Twelve Data API    |
                    |  (REST / JSON)       |
                    +----------+----------+
                               |
                       fetch_candles()
                               |
                    +----------v----------+
                    |  TwelveDataProvider  |
                    |  (DataProvider impl) |
                    |  - pagination        |
                    |  - rate limiting     |
                    |  - retry/backoff     |
                    |  - OHLCV validation  |
                    +----------+----------+
                               |
                          Vec<Candle>
                               |
                    +----------v----------+
                    |     DataFetcher      |
                    |  (orchestrator)      |
                    |  - deduplication     |
                    |  - backfill mode     |
                    |  - incremental mode  |
                    +----+----------+-----+
                         |          |
              write_candles()  upsert_candles()
                         |          |
               +---------v--+  +----v-----------+
               | ParquetStore|  | PostgresStore  |
               | (local fs)  |  | (SQL upsert)   |
               +-------------+  +----------------+
```

## Components

### DataProvider Trait

```rust
pub trait DataProvider {
    async fn fetch_candles(
        &self,
        instrument: Instrument,
        range: DateRange,
    ) -> Result<Vec<Candle>, DataError>;
}
```

The `DataProvider` trait is the abstraction boundary between the fetcher and any external data source. The trait is intentionally minimal -- a single method that takes an instrument and date range and returns candles. This makes it straightforward to swap providers (e.g., for testing with mock data, or to add a second provider like Polygon.io).

### TwelveDataProvider

The concrete implementation for the Twelve Data REST API.

**API endpoint:**

```
GET https://api.twelvedata.com/time_series
    ?symbol={ticker}
    &interval=15min
    &start_date={start}
    &end_date={end}
    &timezone=UTC
    &apikey={key}
    &outputsize=5000
    &format=JSON
```

**Key behaviors:**

- **Pagination / Chunking:** The API returns at most 5,000 data points per request. At ~34 candles/day (DAX worst case: 09:00-17:30 = 34 bars), this covers roughly 147 days. The provider uses a conservative chunk size of 145 days. For a 24-month backfill, this means ~5 sequential API requests per instrument.

- **Rate Limiting:** Uses a `tokio::sync::Semaphore` with 1 permit and an 8-second post-request delay (matching the free tier's 8 requests/minute limit). The `with_rate_limit()` constructor allows custom concurrency and delay for paid tiers.

- **Retry Logic:** Transient errors (HTTP 429, 5xx, network errors) trigger exponential backoff: up to 3 retries with delays of 1s, 2s, 4s. Non-retryable errors (4xx, validation failures) fail immediately.

- **Response Parsing:** The API returns OHLCV values as strings. Each value is parsed into `Decimal` with validation: high >= low, and open/close must fall within the [low, high] range. Invalid candles are logged and skipped rather than failing the entire request.

- **Ordering:** The API returns data newest-first. The provider reverses results to chronological order before returning.

### DataFetcher

The orchestrator that ties a `DataProvider` to storage backends.

**Two modes of operation:**

1. **Full Backfill** (`backfill(instrument, range)`): Fetches all candles in the given date range from the provider, deduplicates by timestamp, and writes to both Parquet and Postgres.

2. **Incremental Fetch** (`incremental(instrument)`): Queries Postgres for the latest stored timestamp, then fetches only newer candles from the provider. Falls back to a 2-year backfill if no existing data is found. Filters out candles at or before the latest timestamp to avoid re-ingesting known data.

**Deduplication:** Before writing, candles are sorted by timestamp and consecutive duplicates (same timestamp) are removed using `Vec::dedup_by_key`. This handles overlap between chunked API responses.

### ParquetStore

Local filesystem storage using Apache Parquet format.

**Partitioning strategy:**

```
data/{instrument_ticker_lower}/{YYYY-MM}.parquet
```

Examples: `data/dax/2024-01.parquet`, `data/ixic/2024-06.parquet`.

Files are partitioned by instrument and calendar month. This granularity balances:
- **Read efficiency:** Backtests typically span months to years; reading a few monthly files is fast.
- **Write granularity:** A single month's data (~700 candles for DAX) fits comfortably in one file.
- **Manageability:** Individual months can be re-fetched without touching other data.

**Schema:**

| Column | Arrow Type | Notes |
|---|---|---|
| `timestamp` | `Timestamp(Millisecond, UTC)` | Bar open time |
| `open` | `Float64` | Stored as f64 for Parquet compatibility |
| `high` | `Float64` | |
| `low` | `Float64` | |
| `close` | `Float64` | |
| `volume` | `Int64` | |
| `instrument` | `Utf8` | Ticker symbol for provenance |

Financial values use `Float64` in Parquet because it is universally supported. On read, values are converted back to `rust_decimal::Decimal` for exact arithmetic. The roundtrip introduces negligible precision loss (< 0.01 for typical index values in the 4,000-40,000 range).

**Write behavior:** Files for a given instrument/month are overwritten entirely. The caller (DataFetcher) is responsible for merging with existing data if incremental writes are needed. Within each file, candles are sorted by timestamp.

**Read behavior:** The `read_candles(instrument, range)` method identifies which monthly files overlap the requested date range, reads each one, filters candles to the exact range, and returns them sorted by timestamp. Missing files are silently skipped (treated as no data for that month).

**Compression:** Snappy compression is used for a good balance of speed and file size.

### PostgresStore

SQL storage using the `candles` table.

**Table schema:**

```sql
CREATE TABLE candles (
    instrument_id TEXT NOT NULL,
    timestamp     TIMESTAMPTZ NOT NULL,
    open          DECIMAL(12,2) NOT NULL,
    high          DECIMAL(12,2) NOT NULL,
    low           DECIMAL(12,2) NOT NULL,
    close         DECIMAL(12,2) NOT NULL,
    volume        BIGINT NOT NULL,
    PRIMARY KEY (instrument_id, timestamp)
);
```

**Upsert strategy:** Uses `INSERT ... ON CONFLICT (instrument_id, timestamp) DO UPDATE SET ...` for idempotent writes. Re-running a backfill for the same date range simply overwrites existing rows with fresh data.

**Batching:** Each INSERT statement includes up to 1,000 rows (7 bind parameters each = 7,000 parameters, well under the Postgres 65,535 limit). Large datasets are chunked automatically.

**Queries:**
- `get_candles(instrument, range)` -- Returns candles ordered by timestamp within the date range.
- `latest_timestamp(instrument)` -- Returns the most recent timestamp for incremental fetch support.

## Error Handling

All data pipeline operations return `Result<_, DataError>`. The `DataError` enum covers:

| Variant | When | Retryable |
|---|---|---|
| `Io` | File read/write failures | No |
| `Parquet` | Parquet encoding/decoding errors | No |
| `Arrow` | Arrow conversion errors | No |
| `Database` | SQLx/Postgres errors | No |
| `Validation` | Bad data (invalid timestamps, OHLCV inconsistencies) | No |
| `Api` | Upstream API error payload | Only if `"server error"` prefix |
| `RateLimited` | HTTP 429 from Twelve Data | Yes |
| `Http` | Network/transport errors (reqwest) | Yes |
| `Json` | JSON deserialization failures | No |

The `DataError::io()` helper associates filesystem paths with I/O errors for actionable diagnostics.

## DST Handling

All timestamps are stored and processed in UTC. The `Instrument` type provides DST-aware conversion via `signal_bar_start_utc(date)`, which uses `chrono-tz` to compute the correct UTC offset for any date.

The Twelve Data API is queried with `timezone=UTC`, so returned timestamps are already in UTC. No timezone conversion is needed during data ingestion. Conversions to local exchange time happen only at display boundaries (dashboard, Telegram messages).

Key DST transitions that affect signal bar UTC times:

| Region | Spring Forward | Fall Back |
|---|---|---|
| Europe/Berlin (DAX) | Last Sunday of March, 02:00 -> 03:00 | Last Sunday of October, 03:00 -> 02:00 |
| Europe/London (FTSE) | Last Sunday of March, 01:00 -> 02:00 | Last Sunday of October, 02:00 -> 01:00 |
| America/New_York (NQ, DOW) | Second Sunday of March, 02:00 -> 03:00 | First Sunday of November, 02:00 -> 01:00 |

These transitions shift the signal bar's UTC time by exactly 1 hour. The engine tests verify correct behavior on the exact transition dates for each region.
