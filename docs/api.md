# API Reference

REST API served by the `sr-engine` binary via `sr-engine serve`. Default bind address: `0.0.0.0:3001`.

## Configuration

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | (required) | PostgreSQL connection string |
| `VALKEY_URL` | (optional) | Valkey/Redis URL for caching |
| `HOST` | `0.0.0.0` | Bind address |
| `PORT` | `3001` | Bind port |

## Middleware

All routes are wrapped with:
- **CORS** — permissive (all origins)
- **Tracing** — HTTP request/response logging via `tower-http`
- **Timeout** — 300s per request (408 on timeout)
- **Compression** — gzip/deflate/brotli

## Response Format

### Success (single item)

```json
{
  "data": { ... }
}
```

### Success (paginated list)

```json
{
  "data": [ ... ],
  "pagination": {
    "page": 0,
    "per_page": 50,
    "total_items": 342,
    "total_pages": 7
  }
}
```

Pagination query params: `?page=0&per_page=50` (page is 0-indexed, per_page max 200).

### Error

```json
{
  "error": {
    "code": "BAD_REQUEST",
    "message": "human-readable description",
    "details": null
  }
}
```

Error codes: `BAD_REQUEST` (400), `NOT_FOUND` (404), `VALIDATION_ERROR` (422), `INTERNAL_ERROR` (500), `DATABASE_ERROR` (500).

---

## Endpoints

### Health

#### `GET /api/health`

Returns service health status.

**Response:** `200 OK`
```json
{"status": "ok"}
```

---

### Backtest

#### `POST /api/backtest/run`

Run a single backtest with given parameters.

**Request body:**
```json
{
  "instrument": "DAX",
  "start_date": "2024-01-01",
  "end_date": "2024-12-31",
  "config": {
    "instrument": "Dax",
    "date_from": "2024-01-01",
    "date_to": "2024-12-31",
    "sl_mode": "SignalBarExtreme",
    "sl_points": 40.0,
    "exit_mode": "EndOfDay"
  }
}
```

**Response:** `200 OK`
```json
{
  "data": {
    "run_id": "uuid",
    "result": { "trades": [...], "stats": {...}, "equity_curve": [...] },
    "duration_ms": 145
  }
}
```

**Errors:** `400` invalid instrument, `404` no candles found, `422` start_date after end_date.

#### `GET /api/backtest/{id}`

Fetch a backtest result by run ID. Checks Valkey cache first, falls back to database.

**Response:** `200 OK` with run summary or cached result.

**Errors:** `404` run not found.

#### `GET /api/backtest/{id}/trades`

Paginated trade list for a backtest run.

**Query params:** `?page=0&per_page=50`

**Response:** `200 OK` with paginated `TradeRow` list.

**Errors:** `404` run not found.

#### `POST /api/backtest/compare`

Run 2-4 configurations side by side and return comparative results.

**Request body:**
```json
{
  "configs": [
    { "instrument": "DAX", "start_date": "2024-01-01", "end_date": "2024-06-30", "config": {...} },
    { "instrument": "DAX", "start_date": "2024-01-01", "end_date": "2024-06-30", "config": {...} }
  ]
}
```

**Response:** `200 OK`
```json
{
  "data": [
    { "run_id": "uuid", "result": {...}, "duration_ms": 120 },
    { "run_id": "uuid", "result": {...}, "duration_ms": 135 }
  ]
}
```

**Errors:** `422` fewer than 2 or more than 4 configs.

#### `GET /api/backtest/history`

List past backtest runs with pagination.

**Query params:** `?page=0&per_page=50`

**Response:** `200 OK` with paginated `BacktestRunSummary` list (id, config_id, instrument_id, dates, total_trades, stats, duration_ms, created_at).

---

### Strategy Configs

#### `POST /api/configs`

Create a new strategy configuration.

**Request body:**
```json
{
  "name": "Conservative DAX",
  "params": { "sl_points": 40, "exit_mode": "EndOfDay" }
}
```

**Response:** `201 Created`
```json
{
  "data": { "id": "uuid" }
}
```

**Errors:** `422` empty name.

#### `GET /api/configs`

List all saved configurations.

**Response:** `200 OK`
```json
{
  "data": [
    { "id": "uuid", "name": "Conservative DAX", "params": {...}, "created_at": "2024-..." }
  ]
}
```

#### `GET /api/configs/{id}`

Fetch a single configuration by ID.

**Response:** `200 OK` with config object.

**Errors:** `404` config not found.

#### `DELETE /api/configs/{id}`

Delete a configuration.

**Response:** `204 No Content`

**Errors:** `404` config not found.

---

### Data

#### `GET /api/data/instruments`

List all available trading instruments.

**Response:** `200 OK`
```json
{
  "data": [
    {
      "id": 1,
      "symbol": "DAX",
      "name": "DAX 40",
      "open_time_local": "09:00",
      "close_time_local": "17:30",
      "timezone": "Europe/Berlin",
      "tick_size": "0.50"
    }
  ]
}
```

#### `GET /api/data/candles`

Query candle data for an instrument within a date range.

**Query params:**
- `instrument` — ticker or name (e.g. `DAX`, `FTSE`, `IXIC`, `DJI`)
- `from` — start date, inclusive (`YYYY-MM-DD`)
- `to` — end date, inclusive (`YYYY-MM-DD`)

**Example:** `GET /api/data/candles?instrument=DAX&from=2024-01-01&to=2024-01-31`

**Response:** `200 OK`
```json
{
  "data": [
    {
      "instrument_id": 1,
      "timestamp": "2024-01-02T08:15:00Z",
      "open": "16800.50",
      "high": "16850.00",
      "low": "16780.25",
      "close": "16830.75",
      "volume": 12345
    }
  ]
}
```

**Errors:** `400` unknown instrument, `422` from after to.

#### `POST /api/data/fetch`

Trigger a data fetch from the external provider. Placeholder — returns immediately with 202.

**Request body:**
```json
{
  "instrument": "DAX",
  "from": "2024-01-01",
  "to": "2024-12-31"
}
```

**Response:** `202 Accepted`
```json
{
  "status": "accepted",
  "message": "Data fetch queued"
}
```

---

### Signals

#### `GET /api/signals/today`

Get all live signals for today's date across all instruments.

**Response:** `200 OK`
```json
{
  "data": [
    {
      "id": "uuid",
      "instrument_id": 1,
      "signal_date": "2024-06-15",
      "signal_bar_high": "16050.00",
      "signal_bar_low": "15980.00",
      "buy_level": "16052.00",
      "sell_level": "15978.00",
      "status": "pending",
      "fill_details": null,
      "created_at": "2024-06-15T08:30:00Z"
    }
  ]
}
```

#### `GET /api/signals/{instrument}/latest`

Get the most recent signal for a specific instrument. The `instrument` path segment accepts any recognized ticker or alias (case-insensitive): `DAX`, `FTSE`, `IXIC`, `DJI`, `nasdaq`, `dow`, etc.

**Response:** `200 OK` with a single `SignalRow`.

**Errors:** `400` unknown instrument, `404` no signal found.
