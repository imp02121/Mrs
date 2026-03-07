# Telegram Bot Reference

Signal notification bot for the School Run Strategy. Delivers real-time alerts when signal bars form, orders trigger, and trades close across DAX, FTSE, NASDAQ, and DOW.

Binary: `sr-telegram`. Built with [Teloxide](https://github.com/teloxide/teloxide).

## Overview

The Telegram bot is a standalone Rust service that connects three systems:

1. **sr-engine HTTP API** -- polled at a configurable interval to detect new or changed signals.
2. **PostgreSQL** -- stores subscriber chat IDs, usernames, and instrument subscriptions in the `subscribers` table.
3. **Telegram Bot API** -- receives user commands and delivers push notifications via Teloxide.

On startup, the bot spawns a background signal polling loop alongside the Teloxide command dispatcher. When a signal changes state (new signal bar, order fill, status update), the bot queries the subscribers table and sends formatted messages to each subscribed user.

## Setup

### 1. Create a Telegram bot

1. Open Telegram and message [@BotFather](https://t.me/BotFather).
2. Send `/newbot` and follow the prompts to choose a name and username.
3. Copy the HTTP API token (e.g. `123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11`).

### 2. Run database migrations

The bot uses the `subscribers` table created by the shared migration set:

```bash
sr-engine migrate
```

### 3. Set environment variables

```bash
export TELEGRAM_BOT_TOKEN="123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"
export DATABASE_URL="postgres://user:pass@localhost/sr"
export ENGINE_API_URL="http://localhost:3001"  # optional, this is the default
```

### 4. Start the bot

```bash
cargo run -p sr-telegram
```

Or via Docker:

```bash
docker build -t sr-telegram -f telegram/Dockerfile .
docker run -e TELEGRAM_BOT_TOKEN=... -e DATABASE_URL=... sr-telegram
```

---

## Configuration

| Variable | Required | Default | Description |
|---|---|---|---|
| `TELEGRAM_BOT_TOKEN` | Yes | -- | Bot API token from BotFather |
| `DATABASE_URL` | Yes | -- | PostgreSQL connection string |
| `ENGINE_API_URL` | No | `http://localhost:3001` | Base URL of the sr-engine HTTP API |
| `VALKEY_URL` | No | -- | Valkey/Redis URL (reserved for future pub/sub) |
| `POLL_INTERVAL_SECS` | No | `30` | Seconds between signal polling cycles |

Logging is controlled by the `RUST_LOG` environment variable (e.g. `RUST_LOG=info` or `RUST_LOG=sr_telegram=debug`).

---

## Commands

| Command | Arguments | Description |
|---|---|---|
| `/start` | -- | Register as a subscriber and show the welcome message |
| `/signals` | -- | Fetch and display today's signals across all instruments |
| `/subscribe` | Comma-separated instruments | Subscribe to notifications for specific instruments |
| `/unsubscribe` | -- | Deactivate your subscription (stop all notifications) |
| `/status` | -- | Show your current subscription status and instruments |

### Examples

**Subscribe to instruments:**
```
/subscribe DAX,FTSE
```
Response:
```
Subscribed to: DAX, FTSE
```

**Subscribe with an invalid instrument:**
```
/subscribe DAX,GOLD,FTSE
```
Response:
```
Subscribed to: DAX, FTSE
Unrecognized (ignored): GOLD
```

**Check subscription status:**
```
/status
```
Response:
```
Status: Active
Instruments: DAX, FTSE
Subscribed since: 2026-03-07
```

**View today's signals:**
```
/signals
```
Response:
```
Today's signals:

DAX: High 22,448.00 / Low 22,390.00 | Buy 22,450.00 / Sell 22,388.00 [pending]
FTSE: High 7,520.00 / Low 7,485.00 | Buy 7,522.00 / Sell 7,483.00 [filled]
```

Valid instrument names: `DAX`, `FTSE`, `NASDAQ`, `DOW` (case-insensitive, duplicates ignored).

---

## Push Notifications

The bot sends four types of push notifications to subscribers. Notifications are only delivered to users who have subscribed to the relevant instrument.

### 1. Signal Bar Formed

Sent when a new signal bar is detected for the first time.

```
[target] DAX Signal Bar Formed
Time: 09:15 - 09:30 CET
High: 22,448.00 | Low: 22,390.00
Buy above: 22,450.00 | Sell below: 22,388.00
```

### 2. Order Triggered

Sent when a signal's status changes to `"filled"`.

```
[zap] DAX LONG Triggered
Entry: 22,450.00 | SL: 22,390.00 | Risk: 60.00 pts
```

### 3. Trade Closed

Sent when a trade exits (stop loss, take profit, or end of day).

```
[check] DAX LONG Closed
Entry: 22,450.00 -> Exit: 22,510.00
PnL: +60.00 points | Duration: 4h 13m
```

### 4. Daily Summary

Sent at end of day with results across all instruments.

```
[chart] Daily Summary - 7 Mar 2026
DAX   LONG  +60 pts
FTSE  SHORT -25 pts
Day Total: +35 points
```

---

## Architecture

```
                    +------------------+
                    |  Telegram Users  |
                    +--------+---------+
                             |
                    Commands | Notifications
                             |
                    +--------v---------+
                    |   sr-telegram    |
                    |                  |
                    |  +------------+  |       +-----------+
                    |  | Dispatcher |  | <---> |  Telegram  |
                    |  | (commands) |  |       |  Bot API   |
                    |  +------------+  |       +-----------+
                    |                  |
                    |  +------------+  |       +-----------+
                    |  | Signal     |  | ----> |  sr-engine |
                    |  | Poller     |  |  GET  |  HTTP API  |
                    |  +------------+  |       +-----------+
                    |                  |
                    +--------+---------+
                             |
                         SQLx queries
                             |
                    +--------v---------+
                    |   PostgreSQL     |
                    |  (subscribers)   |
                    +------------------+
```

### Signal Polling Loop

1. Every `POLL_INTERVAL_SECS` seconds, the bot calls `GET /api/signals/today` on the engine API.
2. A `SignalWatcher` compares the response against previously seen signal states (in-memory `HashMap`).
3. New signals emit a `NewSignal` event; status changes emit a `StatusChanged` event.
4. For each event, the bot queries `subscribers` for active users subscribed to that instrument.
5. Formatted notification messages are sent to each matching subscriber via the Telegram Bot API.

### Command Dispatcher

The Teloxide `Dispatcher` processes incoming messages using `dptree`. Each command is routed to a handler function that interacts with the database and/or engine API, then sends a text response back to the user.

### Subscriber Store

Subscribers are stored in the `subscribers` table (created by the shared migration set):

```sql
CREATE TABLE subscribers (
    id                      SERIAL PRIMARY KEY,
    chat_id                 BIGINT NOT NULL UNIQUE,
    username                TEXT,
    subscribed_instruments  TEXT[] NOT NULL DEFAULT '{}',
    active                  BOOLEAN NOT NULL DEFAULT TRUE,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Store operations:
- `insert_subscriber` -- insert or re-activate (upsert on `chat_id`)
- `get_subscriber` -- fetch by chat ID
- `update_subscriptions` -- set the instrument list
- `list_active_subscribers` -- all active subscribers
- `get_subscribers_for_instrument` -- active subscribers with a specific instrument in their array
- `deactivate_subscriber` -- set `active = false`

---

## Running

### Local development

```bash
# Ensure the engine and database are running first
sr-engine migrate
sr-engine serve &

# Start the bot
TELEGRAM_BOT_TOKEN=<token> \
DATABASE_URL=postgres://localhost/sr \
cargo run -p sr-telegram
```

### Docker

```bash
docker build -t sr-telegram -f telegram/Dockerfile .

docker run \
  -e TELEGRAM_BOT_TOKEN=<token> \
  -e DATABASE_URL=postgres://host.docker.internal/sr \
  -e ENGINE_API_URL=http://host.docker.internal:3001 \
  sr-telegram
```

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `configuration error: TELEGRAM_BOT_TOKEN is required` | Missing env var | Set `TELEGRAM_BOT_TOKEN` before starting |
| `configuration error: DATABASE_URL is required` | Missing env var | Set `DATABASE_URL` before starting |
| `database error: ...` on startup | Cannot connect to PostgreSQL | Verify `DATABASE_URL` is correct and the database is running |
| `failed to poll signals` in logs | Engine API unreachable | Ensure `sr-engine serve` is running and `ENGINE_API_URL` is correct |
| Bot does not respond to commands | Bot token invalid or bot not started | Verify token with BotFather; check logs for startup errors |
| No notifications received | Not subscribed to any instruments | Use `/subscribe DAX,FTSE` to add instruments |
| `invalid POLL_INTERVAL_SECS` | Non-numeric value | Set to a positive integer (e.g. `30`) |

---

## Crate Structure

```
telegram/
  Cargo.toml
  Dockerfile
  src/
    main.rs             # Entry point: config, DB pool, Teloxide dispatcher, signal loop
    lib.rs              # Public module declarations
    config.rs           # Config struct loaded from environment variables
    commands.rs         # Bot commands (start, signals, subscribe, unsubscribe, status)
    notifications.rs    # Message formatters and delivery (4 notification types)
    signals.rs          # Signal polling, change detection (SignalWatcher), API types
    store.rs            # Subscriber CRUD operations (SQLx queries)
    error.rs            # BotError enum (Database, Http, Config, Json, InvalidInstrument)
```

---

## Error Types

The `BotError` enum covers all error conditions:

| Variant | Source | Description |
|---|---|---|
| `Database` | `sqlx::Error` | PostgreSQL connection or query failure |
| `Http` | `reqwest::Error` | Engine API request failure |
| `Config` | -- | Missing or invalid environment variable |
| `Json` | `serde_json::Error` | Response deserialization failure |
| `InvalidInstrument` | -- | Unrecognized instrument name |
