# Data Directory

This directory holds historical OHLCV candle data used by the School Run backtesting engine. All data files in this directory are **gitignored** and must be obtained separately.

## Expected Format

The engine accepts 15-minute OHLCV candle data in **Parquet** format, organized by instrument and month:

```
data/
  dax/
    2024-01.parquet
    2024-02.parquet
    ...
  ftse/
    2024-01.parquet
    ...
  nasdaq/
    2024-01.parquet
    ...
  dow/
    2024-01.parquet
    ...
```

### Column Schema

Each Parquet file must contain the following columns:

| Column | Type | Description |
|---|---|---|
| `timestamp` | `TIMESTAMP` (UTC) | Candle open time in UTC |
| `open` | `DECIMAL(12,2)` | Opening price |
| `high` | `DECIMAL(12,2)` | Highest price during the interval |
| `low` | `DECIMAL(12,2)` | Lowest price during the interval |
| `close` | `DECIMAL(12,2)` | Closing price |
| `volume` | `BIGINT` | Trade volume during the interval |

All timestamps must be in **UTC**. The engine handles timezone conversions internally (e.g., UTC to CET for DAX session times).

### CSV Alternative

During development, the engine also accepts CSV files with the same column schema. CSV files should use comma delimiters and include a header row:

```csv
timestamp,open,high,low,close,volume
2024-01-02T08:00:00Z,16750.50,16765.25,16748.00,16762.75,1250
2024-01-02T08:15:00Z,16762.75,16780.00,16760.50,16775.25,980
```

## Supported Instruments

| Symbol | Instrument | Session Open (Local) | Session Close (Local) | Timezone |
|---|---|---|---|---|
| `DAX` | DAX 40 | 09:00 | 17:30 | Europe/Berlin (CET/CEST) |
| `FTSE` | FTSE 100 | 08:00 | 16:30 | Europe/London (GMT/BST) |
| `NQ` | Nasdaq 100 | 09:30 | 16:00 | America/New_York (EST/EDT) |
| `DOW` | Dow Jones 30 | 09:30 | 16:00 | America/New_York (EST/EDT) |

## Recommended Data Provider

**Twelve Data** is the recommended provider for historical 15-minute candle data:

- Covers all four supported instruments
- 10+ years of intraday history
- Free tier: 800 API calls/day
- Growth plan ($79/month): higher rate limits for bulk backfill

The engine implements a `DataProvider` trait with a Twelve Data implementation. Alternative providers can be added behind the same trait interface.

## Obtaining Data

> Detailed fetch instructions will be added in Phase 1.
>
> Once implemented, the workflow will be:
>
> ```bash
> # Set your API key
> export TWELVE_DATA_API_KEY=your_key_here
>
> # Fetch 24 months of DAX data
> sr-engine fetch DAX
>
> # Fetch all instruments
> sr-engine fetch DAX FTSE NQ DOW
> ```
>
> The fetch command will download candle data, store it as Parquet files in this directory, and optionally ingest it into PostgreSQL.

## Important Notes

- Data files can be large. A single instrument with 24 months of 15-minute data is approximately 34 candles/day x 252 trading days x 2 years = ~17,000 rows.
- The engine expects continuous data without gaps during trading sessions. Missing candles (holidays, data provider outages) should be documented in the `exclude_dates` backtest parameter.
- All financial values use fixed-precision decimal arithmetic. Do not pre-round values in your data.
