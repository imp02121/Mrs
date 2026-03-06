# Database Architecture

PostgreSQL schema for the School Run backtester. Seven tables covering instruments, candle data, strategy configs, backtest results, trades, live signals, and Telegram subscribers.

---

## Schema Overview

```
instruments (SMALLINT PK)
    |
    +--< candles (instrument_id, timestamp) PK  [partitioned by month]
    |
    +--< backtest_runs (instrument_id FK)
    |       |
    |       +--< trades (backtest_run_id FK, ON DELETE CASCADE)
    |
    +--< live_signals (instrument_id FK, UNIQUE instrument_id+signal_date)
    |
    +--- subscribers (independent, no FK to instruments)

strategy_configs (UUID PK)
    |
    +--< backtest_runs (config_id FK)
```

All tables use `TIMESTAMPTZ` for temporal columns and `gen_random_uuid()` for UUID primary keys. Financial values use `NUMERIC(12,2)`.

---

## Tables

### instruments

Reference table for supported trading instruments. Seeded by migration 001 with DAX, FTSE, IXIC, and DJI.

| Column | Type | Notes |
|---|---|---|
| `id` | `SMALLINT` | Auto-identity PK |
| `symbol` | `TEXT UNIQUE` | Ticker (DAX, FTSE, IXIC, DJI) |
| `name` | `TEXT` | Human-readable name |
| `open_time_local` | `TEXT` | Session open in local tz (e.g. "09:00") |
| `close_time_local` | `TEXT` | Session close in local tz |
| `timezone` | `TEXT` | IANA timezone (e.g. "Europe/Berlin") |
| `tick_size` | `NUMERIC(10,2)` | Minimum tick |

### candles

OHLCV candle data. Partitioned by month for efficient range queries.

| Column | Type | Notes |
|---|---|---|
| `instrument_id` | `SMALLINT` | FK to instruments |
| `timestamp` | `TIMESTAMPTZ` | Bar open time (UTC) |
| `open` | `NUMERIC(12,2)` | |
| `high` | `NUMERIC(12,2)` | |
| `low` | `NUMERIC(12,2)` | |
| `close` | `NUMERIC(12,2)` | |
| `volume` | `BIGINT` | Default 0 |

**Primary key:** `(instrument_id, timestamp)` -- composite, inherited by each partition.

**Partitioning:** `PARTITION BY RANGE (timestamp)` with monthly partitions from 2024-01 through 2026-12 (36 partitions). Each partition covers one calendar month. Postgres automatically routes inserts to the correct partition and prunes irrelevant partitions during range queries. This means a backtest querying Jan-Mar 2025 only scans 3 partitions instead of the full table.

**Why monthly partitions:** Backtests typically span months to years. Monthly granularity balances query efficiency (few partitions to scan) against partition count (36 total). Each month holds ~2,800 candles across 4 instruments (700 candles per instrument at 15-min intervals).

**Upsert strategy:** `INSERT ... ON CONFLICT (instrument_id, timestamp) DO UPDATE SET ...` for idempotent writes. Bulk inserts are batched at 1,000 rows per statement (7 params each = 7,000, under the 65,535 Postgres bind limit).

### strategy_configs

Saved strategy parameter sets used for backtests.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` | Default `gen_random_uuid()` |
| `name` | `TEXT` | Human-readable name |
| `params` | `JSONB` | Full strategy parameters |
| `created_at` | `TIMESTAMPTZ` | Default `now()` |

### backtest_runs

Completed backtest executions with summary statistics.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` | Default `gen_random_uuid()` |
| `config_id` | `UUID` | FK to strategy_configs |
| `instrument_id` | `SMALLINT` | FK to instruments |
| `start_date` | `DATE` | Backtest start (inclusive) |
| `end_date` | `DATE` | Backtest end (inclusive) |
| `total_trades` | `INTEGER` | Trade count |
| `stats` | `JSONB` | Summary stats (win rate, PF, Sharpe, etc.) |
| `duration_ms` | `INTEGER` | Wall-clock execution time |
| `created_at` | `TIMESTAMPTZ` | Default `now()` |

### trades

Individual trades belonging to a backtest run. Cascade-deleted when the parent run is removed.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` | Default `gen_random_uuid()` |
| `backtest_run_id` | `UUID` | FK to backtest_runs, `ON DELETE CASCADE` |
| `instrument_id` | `SMALLINT` | FK to instruments |
| `direction` | `TEXT` | CHECK: 'Long' or 'Short' |
| `entry_price` | `NUMERIC(12,2)` | |
| `entry_time` | `TIMESTAMPTZ` | |
| `exit_price` | `NUMERIC(12,2)` | |
| `exit_time` | `TIMESTAMPTZ` | |
| `stop_loss` | `NUMERIC(12,2)` | |
| `exit_reason` | `TEXT` | e.g. StopLoss, EndOfDay, TrailingStop |
| `pnl_points` | `NUMERIC(12,2)` | Base position PnL |
| `pnl_with_adds` | `NUMERIC(12,2)` | Total PnL including add-ons |
| `adds` | `JSONB` | Add-on position details, default `'[]'` |
| `trade_date` | `DATE` | Calendar date of trade |

Bulk inserts use 1,000-row batches (13 params each).

### live_signals

Daily trading signals for live monitoring.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` | Default `gen_random_uuid()` |
| `instrument_id` | `SMALLINT` | FK to instruments |
| `signal_date` | `DATE` | |
| `signal_bar_high` | `NUMERIC(12,2)` | |
| `signal_bar_low` | `NUMERIC(12,2)` | |
| `buy_level` | `NUMERIC(12,2)` | Buy stop price |
| `sell_level` | `NUMERIC(12,2)` | Sell stop price |
| `status` | `TEXT` | Default 'pending' |
| `fill_details` | `JSONB` | Optional fill info |
| `created_at` | `TIMESTAMPTZ` | Default `now()` |

**Unique constraint:** `(instrument_id, signal_date)` -- one signal per instrument per day. Upserts use `ON CONFLICT ... DO UPDATE`.

### subscribers

Telegram bot subscribers for signal notifications.

| Column | Type | Notes |
|---|---|---|
| `id` | `SERIAL` | Auto-increment PK |
| `chat_id` | `BIGINT UNIQUE` | Telegram chat ID |
| `username` | `TEXT` | Optional |
| `subscribed_instruments` | `TEXT[]` | Array of ticker symbols |
| `active` | `BOOLEAN` | Default `true` |
| `created_at` | `TIMESTAMPTZ` | Default `now()` |

---

## Indexes

| Index | Table | Columns | Notes |
|---|---|---|---|
| PK | instruments | `id` | Auto |
| UNIQUE | instruments | `symbol` | |
| PK (partitioned) | candles | `(instrument_id, timestamp)` | Per-partition |
| `idx_backtest_runs_config_id` | backtest_runs | `config_id` | FK lookup |
| `idx_backtest_runs_instrument_id` | backtest_runs | `instrument_id` | FK lookup |
| `idx_backtest_runs_created_at` | backtest_runs | `created_at DESC` | Paginated listing |
| `idx_trades_backtest_run_id` | trades | `backtest_run_id` | FK lookup, cascade delete |
| `idx_trades_instrument_id` | trades | `instrument_id` | Filter by instrument |
| `idx_trades_trade_date` | trades | `trade_date` | Date range queries |
| `idx_live_signals_signal_date` | live_signals | `signal_date DESC` | Latest signal lookup |
| UNIQUE | live_signals | `(instrument_id, signal_date)` | One signal per instrument per day |
| UNIQUE | subscribers | `chat_id` | Telegram ID lookup |
| `idx_subscribers_active` | subscribers | `active` WHERE `active = true` | Partial index for active-only queries |

---

## Migrations

Seven SQL migration files in `migrations/`:

| File | Purpose |
|---|---|
| `001_create_instruments.sql` | Create instruments table + seed 4 instruments |
| `002_create_candles.sql` | Create partitioned candles table + 36 monthly partitions |
| `003_create_strategy_configs.sql` | Create strategy_configs table |
| `004_create_backtest_runs.sql` | Create backtest_runs table + indexes |
| `005_create_trades.sql` | Create trades table + indexes |
| `006_create_live_signals.sql` | Create live_signals table + indexes |
| `007_create_subscribers.sql` | Create subscribers table + partial index |

### Running Migrations

```bash
export DATABASE_URL="postgres://user:pass@localhost:5432/sr"
sqlx migrate run
```

SQLx reads migrations from the `migrations/` directory and applies them in order. Each migration is tracked in the `_sqlx_migrations` table to ensure idempotent execution.

### Adding Partitions

The candle partitions cover through 2026-12. To extend coverage, add new partition DDL:

```sql
CREATE TABLE IF NOT EXISTS candles_2027_01
    PARTITION OF candles
    FOR VALUES FROM ('2027-01-01') TO ('2027-02-01');
```

---

## Connection Configuration

Set `DATABASE_URL` as an environment variable:

```
DATABASE_URL=postgres://user:password@host:5432/sr
```

The engine uses `sqlx::PgPool` for async connection pooling. Pool size defaults to SQLx's automatic configuration based on available connections.

---

## Rust Query Layer

All database queries live in `engine/src/db/`. Each table has a dedicated module with typed row structs (`sqlx::FromRow`) and query functions:

| Module | Functions |
|---|---|
| `instruments.rs` | `get_instrument_by_symbol`, `list_instruments`, `get_instrument_id` |
| `candles.rs` | `upsert_candles`, `get_candles`, `latest_timestamp`, `count_candles` |
| `configs.rs` | `insert_config`, `get_config`, `list_configs`, `delete_config` |
| `backtests.rs` | `insert_backtest_run`, `get_backtest_run`, `list_backtest_runs`, `delete_backtest_run` |
| `trades.rs` | `insert_trades`, `get_trades_for_run`, `count_trades_for_run` |
| `signals.rs` | `upsert_signal`, `get_latest_signal`, `get_today_signals` |
| `subscribers.rs` | `insert_subscriber`, `get_subscriber`, `update_subscriptions`, `list_active_subscribers`, `deactivate_subscriber` |
