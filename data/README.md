# Data Directory

This directory holds historical OHLCV candle data used by the School Run backtesting engine. All data files in this directory are **gitignored** and must be obtained separately.

## Quick Start

```bash
# 1. Get a Twelve Data API key from https://twelvedata.com
# 2. Set it as an environment variable
export TWELVE_DATA_API_KEY=your_key_here

# 3. Fetch 24 months of data for a single instrument
sr-engine fetch --instrument DAX --months 24

# 4. Fetch all four supported instruments
sr-engine fetch --instrument DAX --months 24
sr-engine fetch --instrument FTSE --months 24
sr-engine fetch --instrument NQ --months 24
sr-engine fetch --instrument DOW --months 24
```

## Twelve Data API Key Setup

1. Create a free account at [twelvedata.com](https://twelvedata.com).
2. Navigate to your dashboard and copy the API key.
3. Export it in your shell:

   ```bash
   export TWELVE_DATA_API_KEY=your_key_here
   ```

   Or add it to your `.env` file (gitignored):

   ```
   TWELVE_DATA_API_KEY=your_key_here
   ```

### API Tiers

| Tier | Rate Limit | Daily Limit | Cost |
|---|---|---|---|
| **Free** | 8 requests/min | 800 requests/day | $0 |
| **Growth** | 30 requests/min | Unlimited | $79/month |

The engine respects rate limits automatically. On the free tier, each API request is throttled with an 8-second inter-request delay (1 permit, 8s cooldown). You can configure custom rate limits programmatically via `TwelveDataProvider::with_rate_limit()`.

## Expected File Structure

Parquet files are organized by instrument (lowercased ticker) and month:

```
data/
  dax/
    2024-01.parquet
    2024-02.parquet
    ...
  ftse/
    2024-01.parquet
    ...
  ixic/
    2024-01.parquet
    ...
  dji/
    2024-01.parquet
    ...
```

Each directory name matches the lowercased ticker symbol from the Twelve Data API: `dax`, `ftse`, `ixic` (Nasdaq), `dji` (Dow).

### Parquet Column Schema

Each Parquet file uses Snappy compression and contains the following columns:

| Column | Arrow Type | Description |
|---|---|---|
| `timestamp` | `Timestamp(Millisecond, UTC)` | Candle open time in UTC |
| `open` | `Float64` | Opening price |
| `high` | `Float64` | Highest price during the 15-minute interval |
| `low` | `Float64` | Lowest price during the 15-minute interval |
| `close` | `Float64` | Closing price |
| `volume` | `Int64` | Trade volume during the interval |
| `instrument` | `Utf8` | Ticker symbol (e.g. `"DAX"`, `"FTSE"`) |

All timestamps are in **UTC**. The engine handles timezone conversions internally (e.g., UTC to CET for DAX session times).

Financial values are stored as `Float64` in Parquet for broad compatibility. They are converted to `rust_decimal::Decimal` on read for exact arithmetic in the engine. Minor floating-point precision loss (< 0.01) may occur during the roundtrip.

### CSV Alternative

During development, the engine also accepts CSV files with the same column schema. CSV files should use comma delimiters and include a header row:

```csv
timestamp,open,high,low,close,volume
2024-01-02T08:00:00Z,16750.50,16765.25,16748.00,16762.75,1250
2024-01-02T08:15:00Z,16762.75,16780.00,16760.50,16775.25,980
```

## Supported Instruments

| CLI Name | Ticker | Index | Session (Local) | Timezone |
|---|---|---|---|---|
| `DAX` | `DAX` | DAX 40 | 09:00 - 17:30 | Europe/Berlin (CET/CEST) |
| `FTSE` | `FTSE` | FTSE 100 | 08:00 - 16:30 | Europe/London (GMT/BST) |
| `NQ` | `IXIC` | Nasdaq Composite | 09:30 - 16:00 | America/New_York (EST/EDT) |
| `DOW` | `DJI` | Dow Jones 30 | 09:30 - 16:00 | America/New_York (EST/EDT) |

## Rate Limiting and Backfill Timing

The Twelve Data API returns a maximum of 5,000 data points per request. The engine automatically splits long date ranges into chunks of 145 days (conservative margin for ~34 candles/day) and issues sequential paginated requests.

### Estimated Backfill Times

Assuming ~34 candles/day and ~252 trading days/year:

| Range | Candles/Instrument | API Requests | Free Tier Time | Growth Tier Time |
|---|---|---|---|---|
| 6 months | ~4,300 | 1 | ~10 seconds | ~5 seconds |
| 12 months | ~8,600 | 2 | ~25 seconds | ~10 seconds |
| 24 months | ~17,200 | 4 | ~45 seconds | ~15 seconds |

Times are approximate and include rate-limit delays and network latency. Backfilling all four instruments for 24 months on the free tier takes roughly 3 minutes.

### Retry Behavior

The engine retries transient errors (HTTP 429 rate limits, 5xx server errors) with exponential backoff:

- Maximum 3 retries per request
- Base delay: 1 second, doubling on each retry (1s, 2s, 4s)
- HTTP 429 responses include a `retry-after` header that is respected

## Data Pipeline Architecture

The data pipeline has three layers:

1. **Provider** (`DataProvider` trait / `TwelveDataProvider`): Fetches raw OHLCV data from the Twelve Data REST API. Handles pagination, rate limiting, retry logic, and response parsing with OHLCV validation.

2. **Storage**:
   - **Parquet** (`ParquetStore`): Local file storage partitioned by instrument and month. Used for offline backtesting without a database.
   - **PostgreSQL** (`PostgresStore`): Bulk upsert into the `candles` table using `INSERT ... ON CONFLICT` for idempotent writes. Batched at 1,000 rows per INSERT to stay within Postgres parameter limits.

3. **Orchestrator** (`DataFetcher`): Coordinates fetching from a provider, deduplicating by timestamp, and writing to both Parquet and Postgres. Supports both full backfill and incremental fetch modes.

## PostgreSQL Storage

The engine can also ingest candles into PostgreSQL for use by the HTTP API and dashboard. The `candles` table uses a composite unique constraint on `(instrument_id, timestamp)` for idempotent upserts.

To use PostgreSQL storage, ensure:
- The database is running and migrations have been applied (`sr-engine migrate`)
- The `DATABASE_URL` environment variable is set

## DST (Daylight Saving Time) Handling

All timestamps are stored and processed in **UTC**. The engine uses `chrono-tz` for DST-aware conversions when determining signal bar times:

| Instrument | Winter (Standard) | Summer (Daylight) |
|---|---|---|
| DAX | Signal bar at 08:15 UTC (CET, UTC+1) | Signal bar at 07:15 UTC (CEST, UTC+2) |
| FTSE | Signal bar at 08:15 UTC (GMT, UTC+0) | Signal bar at 07:15 UTC (BST, UTC+1) |
| Nasdaq/Dow | Signal bar at 14:45 UTC (EST, UTC-5) | Signal bar at 13:45 UTC (EDT, UTC-4) |

DST transitions happen at different dates for European and US markets. The engine computes the correct UTC offset per date using the IANA timezone database, so you do not need to manually account for clock changes.

## Troubleshooting

### Rate Limit Errors

```
Error: rate limited, retry after 60s
```

You have exceeded the Twelve Data API rate limit. The engine retries automatically, but if this persists:
- Wait a few minutes and try again
- Check your API tier (free tier: 800 calls/day)
- Reduce the date range and fetch in smaller batches
- Upgrade to the Growth tier for higher limits

### Missing Data / Empty Results

```
Warning: provider returned no candles
```

Possible causes:
- The date range falls entirely on weekends or holidays
- The instrument ticker is not covered by your API plan
- Twelve Data may have maintenance windows

Check: verify the date range includes trading days for the instrument.

### DST Gaps in Data

Around DST transitions, the number of candles per day changes slightly because the UTC-mapped session boundaries shift. This is expected behavior, not a data gap. The engine handles this correctly.

### Parquet Read Errors

```
Error: parquet error: ...
```

A Parquet file may be corrupted (e.g., from a partial write during a crash). Delete the affected file in `data/{instrument}/{YYYY-MM}.parquet` and re-fetch:

```bash
sr-engine fetch --instrument DAX --months 1
```

### Network Errors

The engine retries HTTP errors up to 3 times with exponential backoff. If fetches consistently fail:
- Check your internet connection
- Verify the API key is valid: `curl "https://api.twelvedata.com/time_series?symbol=DAX&interval=15min&outputsize=1&apikey=$TWELVE_DATA_API_KEY"`
- Check Twelve Data's status page for outages

## Important Notes

- Data files can be large. A single instrument with 24 months of 15-minute data is approximately 34 candles/day x 252 trading days x 2 years = ~17,000 rows.
- The engine expects continuous data without gaps during trading sessions. Missing candles (holidays, data provider outages) should be documented in the `exclude_dates` backtest parameter.
- All financial values use fixed-precision decimal arithmetic internally. Do not pre-round values in your data.
- The Parquet store overwrites existing monthly files on write. For incremental updates, use the `DataFetcher::incremental()` method which fetches only candles newer than the latest stored timestamp.
