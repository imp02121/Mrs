# Cache Architecture

Valkey (Redis-compatible) cache layer for the School Run backtester. Provides fast reads for hot data and a write-behind pipeline for asynchronous Postgres persistence.

---

## Key Patterns

All keys are prefixed with `sr:` to namespace within the Valkey instance.

| Key Pattern | Value | TTL | Purpose |
|---|---|---|---|
| `sr:signal:{ticker}:latest` | JSON signal data | 24h | Latest signal per instrument |
| `sr:backtest:{run_id}:result` | JSON `BacktestResult` | 7d | Full cached backtest result |
| `sr:backtest:{run_id}:progress` | JSON `{progress, status}` | 1h | Backtest progress tracking |
| `sr:trades:{ticker}:today` | JSON trades array | 24h | Today's trades (reserved) |
| `sr:tracking:{ticker}:{date}` | JSON signal state | 24h | Telegram bot state (reserved) |

Tickers use the instrument symbol: `DAX`, `FTSE`, `IXIC`, `DJI`.

---

## Components

### ValkeyCache

`engine/src/db/cache.rs`

Core cache client wrapping a `redis::aio::ConnectionManager`. Provides:

- `set_json<T>(key, value, ttl)` -- serialize any `Serialize` type to JSON and store with TTL
- `get_json<T>(key)` -- deserialize from cache, returns `None` on miss
- `delete(key)` -- remove a key

Domain-specific helpers built on these primitives:

```rust
cache.set_backtest_result(run_id, &result).await?;  // 7-day TTL
cache.get_backtest_result(run_id).await?;

cache.set_signal(Instrument::Dax, &signal_json).await?;  // 24h TTL
cache.get_latest_signal(Instrument::Dax).await?;

cache.set_backtest_progress(run_id, 0.75, "running").await?;  // 1h TTL
cache.get_backtest_progress(run_id).await?;
```

Static key builders for external use: `ValkeyCache::backtest_result_key(run_id)`, `signal_key(instrument)`, `progress_key(run_id)`.

### WriteBehindWorker

`engine/src/db/write_behind.rs`

Background Tokio task that batches write operations and flushes them to Postgres asynchronously.

```
Application Code
      |
      | WriteBehindSender::send(WriteOp)
      v
  mpsc channel (bounded)
      |
      | drain loop
      v
WriteBehindWorker
      |
      | batch flush
      v
   PostgreSQL
```

**Flush triggers:** Every 500ms or every 100 buffered items, whichever comes first.

**Supported operations (`WriteOp` enum):**

| Variant | What it does |
|---|---|
| `InsertBacktestRun` | Creates a `strategy_configs` row, then inserts into `backtest_runs` with upsert |
| `InsertTrades` | Logs intent for bulk trade insert (deferred to `db::trades::insert_trades`) |
| `UpsertSignal` | Resolves `instrument_id`, then upserts into `live_signals` |

**Lifecycle:**

1. Call `WriteBehindWorker::new(pool, buffer_size)` to get a `(worker, sender)` pair
2. Spawn `worker.run()` on a Tokio task
3. Clone `sender` and pass to application code
4. Worker shuts down gracefully when all senders are dropped (channel closes), flushing remaining items

**Error handling:** Individual operation failures are logged but do not stop the worker. The batch continues processing remaining operations.

### CacheReader

`engine/src/db/cache_reader.rs`

Implements the cache-aside (cache-first) pattern:

```
Read Request
      |
      v
  Check Valkey ──hit──> Return cached value
      |
      miss (or error)
      |
      v
  Query Postgres
      |
      v
  Backfill Valkey (best-effort)
      |
      v
  Return value
```

**Available reads:**

- `get_backtest_result(run_id)` -- checks cache, falls back to Postgres `backtest_runs.stats`. Note: the full `BacktestResult` (with trades, equity curve) is only available from cache. The Postgres fallback can confirm the run exists but cannot reconstruct the full result.

- `get_latest_signal(instrument)` -- checks cache, falls back to Postgres `live_signals` table (most recent by `signal_date`). On Postgres hit, backfills the cache with a 24h TTL.

---

## Failure Modes

| Scenario | Behavior |
|---|---|
| Valkey down on read | Warning logged, falls back to Postgres transparently |
| Valkey down on write | `CacheError::Connection` returned to caller |
| Valkey down on backfill | Warning logged, read still returns Postgres data |
| Postgres down (write-behind) | Error logged per operation, worker continues running |
| mpsc channel full | `WriteBehindSender::send()` awaits until space is available |
| All senders dropped | Worker flushes remaining buffer, then exits |

The system degrades gracefully when Valkey is unavailable. Reads fall back to Postgres (slower but correct). Writes via the write-behind pipeline will fail per-operation but the worker stays alive, so transient Postgres issues are survivable.

---

## Connection Configuration

Set `VALKEY_URL` as an environment variable:

```
VALKEY_URL=redis://localhost:6379
```

`ValkeyCache::new(url)` establishes a `ConnectionManager` that automatically reconnects on connection loss.

---

## Error Types

### CacheError

`engine/src/db/cache_error.rs`

| Variant | When |
|---|---|
| `Connection` | Redis/Valkey communication failure |
| `Serialization` | JSON serialize/deserialize failure |

### ReaderError

`engine/src/db/cache_reader.rs`

| Variant | When |
|---|---|
| `Cache` | Wraps `CacheError` |
| `Database` | SQLx/Postgres query failure |
| `Deserialization` | JSON conversion failure |
