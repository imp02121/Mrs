# Deployment Guide

Step-by-step instructions for deploying the School Run services with Docker. Each service runs as an independent container -- there is no docker-compose coupling.

## Prerequisites

| Dependency | Minimum Version | Notes |
|---|---|---|
| Docker | 24+ | Build and run all service images |
| PostgreSQL | 16 | Shared database for all services |
| Valkey | 8 | Redis-compatible cache (optional but recommended) |

## Environment Variables

### sr-engine

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | Yes | -- | PostgreSQL connection string (e.g. `postgres://user:pass@host/sr`) |
| `VALKEY_URL` | No | -- | Valkey/Redis URL for caching (e.g. `redis://host:6379`) |
| `HOST` | No | `0.0.0.0` | Bind address |
| `PORT` | No | `3001` | Bind port |
| `DATA_PROVIDER_API_KEY` | No | -- | Twelve Data API key (required for data fetching) |
| `RUST_LOG` | No | `info` | Log level (`debug`, `info`, `warn`, `error`) |

### sr-auth

| Variable | Required | Default | Description |
|---|---|---|---|
| `AUTH_DATABASE_URL` | Yes | -- | PostgreSQL connection string |
| `JWT_SECRET` | Yes | -- | HS256 signing secret (min 32 bytes) |
| `RESEND_API_KEY` | No | -- | Resend transactional email API key. Omit for dev mode (OTP logged to console) |
| `RESEND_FROM` | No | `School Run <onboarding@resend.dev>` | Email sender address |
| `HOST` | No | `0.0.0.0` | Bind address |
| `PORT` | No | `3002` | Bind port |
| `OTP_TTL_SECONDS` | No | `300` | OTP expiry time in seconds |
| `RUST_LOG` | No | `info` | Log level |

### sr-telegram

| Variable | Required | Default | Description |
|---|---|---|---|
| `TELEGRAM_BOT_TOKEN` | Yes | -- | Bot API token from [@BotFather](https://t.me/BotFather) |
| `DATABASE_URL` | Yes | -- | PostgreSQL connection string |
| `ENGINE_API_URL` | No | `http://localhost:3001` | Base URL of the sr-engine HTTP API |
| `VALKEY_URL` | No | -- | Valkey/Redis URL (reserved for future pub/sub) |
| `POLL_INTERVAL_SECS` | No | `30` | Seconds between signal polling cycles |
| `RUST_LOG` | No | `info` | Log level |

### Dashboard (build-time only)

| Variable | Required | Default | Description |
|---|---|---|---|
| `VITE_API_URL` | No | `/api` | Engine API base URL (baked into the build) |
| `VITE_AUTH_URL` | No | `/auth` | Auth service base URL (baked into the build) |

Dashboard environment variables are embedded at build time by Vite. They cannot be changed after the image is built. To change them, rebuild the image with the new values:

```bash
docker build --build-arg VITE_API_URL=https://api.example.com \
  -t sr-dashboard -f dashboard/Dockerfile dashboard/
```

---

## Quick Start

All commands are run from the repository root.

### 1. Start PostgreSQL

```bash
docker run -d --name sr-postgres \
  -e POSTGRES_USER=sr \
  -e POSTGRES_PASSWORD=sr_dev \
  -e POSTGRES_DB=school_run \
  -p 5432:5432 \
  postgres:16-alpine
```

### 2. Start Valkey

```bash
docker run -d --name sr-valkey \
  -p 6379:6379 \
  valkey/valkey:8-alpine
```

### 3. Build and run the engine

```bash
docker build -t sr-engine -f engine/Dockerfile .

docker run -d --name sr-engine \
  -e DATABASE_URL=postgres://sr:sr_dev@host.docker.internal:5432/school_run \
  -e VALKEY_URL=redis://host.docker.internal:6379 \
  -p 3001:3001 \
  sr-engine
```

Run database migrations:

```bash
docker exec sr-engine sr-engine migrate
```

Optionally fetch historical data:

```bash
docker exec -e DATA_PROVIDER_API_KEY=your_key sr-engine sr-engine fetch DAX --months 24
```

### 4. Build and run auth

```bash
docker build -t sr-auth -f auth/Dockerfile .

docker run -d --name sr-auth \
  -e AUTH_DATABASE_URL=postgres://sr:sr_dev@host.docker.internal:5432/school_run \
  -e JWT_SECRET=change-this-to-a-random-string-at-least-32-bytes \
  -p 3002:3002 \
  sr-auth
```

Seed an allowed email (required before anyone can log in):

```bash
docker exec sr-postgres psql -U sr -d school_run \
  -c "INSERT INTO allowed_emails (email, role) VALUES ('you@example.com', 'admin');"
```

### 5. Build and run the dashboard

```bash
docker build -t sr-dashboard -f dashboard/Dockerfile dashboard/

docker run -d --name sr-dashboard \
  -p 3000:80 \
  sr-dashboard
```

The dashboard is now accessible at `http://localhost:3000`. The nginx reverse proxy forwards `/api/` requests to the engine.

### 6. Build and run telegram (optional)

```bash
docker build -t sr-telegram -f telegram/Dockerfile .

docker run -d --name sr-telegram \
  -e TELEGRAM_BOT_TOKEN=your_bot_token \
  -e DATABASE_URL=postgres://sr:sr_dev@host.docker.internal:5432/school_run \
  -e ENGINE_API_URL=http://host.docker.internal:3001 \
  sr-telegram
```

### Verify

```bash
# Engine health
curl http://localhost:3001/api/health
# Expected: {"status":"ok"}

# Auth (returns 401 without a token, confirming it is running)
curl http://localhost:3002/auth/me
# Expected: 401 Unauthorized

# Dashboard
curl -s -o /dev/null -w '%{http_code}' http://localhost:3000
# Expected: 200
```

---

## Individual Service Details

### Engine (`sr-engine`)

| Property | Value |
|---|---|
| Dockerfile | `engine/Dockerfile` |
| Base image | `rust:1.85-slim` (builder), `debian:bookworm-slim` (runtime) |
| Binary | `/usr/local/bin/sr-engine` |
| Default port | 3001 |
| Health check | `GET /api/health` |
| Includes migrations | Yes (`/app/migrations/`) |

**Build:**
```bash
docker build -t sr-engine -f engine/Dockerfile .
```

**Run:**
```bash
docker run -d --name sr-engine \
  -e DATABASE_URL=postgres://... \
  -e VALKEY_URL=redis://... \
  -p 3001:3001 \
  sr-engine
```

**Migrate:**
```bash
docker exec sr-engine sr-engine migrate
```

### Auth (`sr-auth`)

| Property | Value |
|---|---|
| Dockerfile | `auth/Dockerfile` |
| Base image | `rust:1.85-slim` (builder), `debian:bookworm-slim` (runtime) |
| Binary | `/usr/local/bin/sr-auth` |
| Default port | 3002 |
| Health check | `GET /auth/me` (returns 401 without token) |

**Build:**
```bash
docker build -t sr-auth -f auth/Dockerfile .
```

**Run:**
```bash
docker run -d --name sr-auth \
  -e AUTH_DATABASE_URL=postgres://... \
  -e JWT_SECRET=your-secret-here \
  -p 3002:3002 \
  sr-auth
```

### Dashboard

| Property | Value |
|---|---|
| Dockerfile | `dashboard/Dockerfile` |
| Base image | `node:22-alpine` (builder), `nginx:alpine` (runtime) |
| Default port | 80 (mapped to host port of your choice) |
| nginx config | `dashboard/nginx.conf` |
| Health check | `GET /` (returns 200) |

**Build:**
```bash
docker build -t sr-dashboard -f dashboard/Dockerfile dashboard/
```

**Run:**
```bash
docker run -d --name sr-dashboard -p 3000:80 sr-dashboard
```

The nginx config handles:
- SPA routing (`try_files $uri $uri/ /index.html`)
- API proxying (`/api/` forwarded to `http://engine:3001`)
- Standard proxy headers (`X-Real-IP`, `X-Forwarded-For`, `X-Forwarded-Proto`)

### Telegram (`sr-telegram`)

| Property | Value |
|---|---|
| Dockerfile | `telegram/Dockerfile` |
| Base image | `rust:1.85-slim` (builder), `debian:bookworm-slim` (runtime) |
| Binary | `/usr/local/bin/sr-telegram` |
| Default port | None (outbound only) |
| Health check | Check logs for `starting signal polling loop` |

**Build:**
```bash
docker build -t sr-telegram -f telegram/Dockerfile .
```

**Run:**
```bash
docker run -d --name sr-telegram \
  -e TELEGRAM_BOT_TOKEN=... \
  -e DATABASE_URL=postgres://... \
  -e ENGINE_API_URL=http://engine-host:3001 \
  sr-telegram
```

---

## Updating Services

Each service can be rebuilt and restarted independently without affecting others.

```bash
# Example: update the engine
docker stop sr-engine && docker rm sr-engine

docker build -t sr-engine -f engine/Dockerfile .

docker run -d --name sr-engine \
  -e DATABASE_URL=postgres://... \
  -e VALKEY_URL=redis://... \
  -p 3001:3001 \
  sr-engine

# Run any new migrations
docker exec sr-engine sr-engine migrate
```

The multi-stage Dockerfiles cache dependency builds. If only source code changes (no new crate dependencies), rebuilds are fast because the dependency layer is reused.

---

## Networking

The default `nginx.conf` proxies `/api/` requests to `http://engine:3001`. This hostname works when containers share a Docker network:

```bash
# Create a shared network
docker network create sr-net

# Start containers on the network
docker run -d --name sr-postgres --network sr-net ...
docker run -d --name sr-valkey --network sr-net ...
docker run -d --name engine --network sr-net ...
docker run -d --name sr-auth --network sr-net ...
docker run -d --name sr-dashboard --network sr-net -p 3000:80 sr-dashboard
docker run -d --name sr-telegram --network sr-net ...
```

With a shared network, containers can reach each other by name. The dashboard's nginx proxies to `http://engine:3001`, so the engine container must be named `engine` (or edit `nginx.conf` to match your container name).

If running without a shared network (e.g. all containers on the host network), use `host.docker.internal` or the host's IP address in environment variables.

---

## Production Considerations

### TLS Termination

The services do not handle TLS themselves. Place a reverse proxy (nginx, Caddy, Traefik) in front of the dashboard and API:

```
Internet -> TLS proxy (:443) -> sr-dashboard (:80) -> nginx -> sr-engine (:3001)
                              -> sr-auth (:3002)
```

### Secrets Management

- Never commit `.env` files or secrets to version control.
- Use Docker secrets, environment files outside the repo, or a secrets manager (e.g. AWS Secrets Manager, HashiCorp Vault).
- The `JWT_SECRET` must be shared between `sr-auth` and `sr-engine` for token validation.
- Generate strong secrets: `openssl rand -base64 48`

### Database Backups

```bash
# Dump the database
docker exec sr-postgres pg_dump -U sr school_run > backup_$(date +%Y%m%d).sql

# Restore from backup
docker exec -i sr-postgres psql -U sr school_run < backup_20260307.sql
```

Consider automated daily backups with `pg_dump` via cron or a managed PostgreSQL service with built-in backups.

### Monitoring and Logging

All Rust services use `tracing` for structured logging. Control verbosity with the `RUST_LOG` environment variable:

```bash
# Service-level debug logging
RUST_LOG=sr_engine=debug

# All debug logging
RUST_LOG=debug

# Quiet mode (errors only)
RUST_LOG=error
```

Monitor service health:
- **Engine**: `GET /api/health` returns `{"status":"ok"}`
- **Auth**: `GET /auth/me` returns 401 when healthy (no token provided)
- **Dashboard**: `GET /` returns 200
- **Telegram**: Check container logs for `starting signal polling loop`

### Resource Limits

Set memory and CPU limits on containers to prevent resource exhaustion:

```bash
docker run -d --name sr-engine \
  --memory=512m --cpus=1 \
  ...
```

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `connection refused` on port 5432 | PostgreSQL not running or wrong host | Verify `sr-postgres` container is running; use `host.docker.internal` from other containers |
| `connection refused` on port 3001 | Engine not running or wrong port mapping | Check `docker ps` for the engine container; verify `-p 3001:3001` |
| `relation "candles" does not exist` | Migrations not run | Run `docker exec sr-engine sr-engine migrate` |
| Dashboard shows blank page | API URL mismatch | Rebuild dashboard with correct `VITE_API_URL` or check nginx proxy config |
| `CORS` errors in browser console | Engine not reachable from dashboard origin | Ensure `/api/` is proxied through nginx, not called directly |
| `invalid JWT secret` | `JWT_SECRET` mismatch between auth and engine | Use the same `JWT_SECRET` value for both services |
| Engine returns 404 for candles | No data fetched yet | Run `docker exec sr-engine sr-engine fetch DAX --months 24` |
| Telegram bot not responding | Invalid bot token | Verify `TELEGRAM_BOT_TOKEN` with [@BotFather](https://t.me/BotFather) |
| Slow Docker builds | No dependency cache | Ensure Dockerfile uses the multi-stage pattern; don't change `Cargo.toml` unnecessarily |
| Port already in use | Another process on the same port | Stop the conflicting process or map to a different host port (e.g. `-p 3002:3001`) |
