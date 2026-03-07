# Auth Service Reference

Standalone OTP-based authentication microservice for the School Run dashboard. No signup flow -- only pre-approved email addresses in PostgreSQL can authenticate.

Binary: `sr-auth`. Default bind address: `0.0.0.0:3002`.

## Overview

The auth service is a separate Rust/Axum microservice (`auth/` crate) that gates access to the dashboard. Authentication is one-time password (OTP) only -- no passwords. A user requests an OTP via email, verifies it, and receives a JWT that grants access to the engine API.

Key properties:

- **Whitelist-based**: Only emails in the `allowed_emails` table can log in.
- **No signup**: Admins add users directly via SQL (`INSERT INTO allowed_emails ...`).
- **Anti-enumeration**: All endpoints return identical responses regardless of whether an email exists.
- **Stateless sessions**: JWTs are not stored server-side. Logout is client-only.

## Configuration

| Variable | Required | Default | Description |
|---|---|---|---|
| `AUTH_DATABASE_URL` | Yes | -- | PostgreSQL connection string |
| `JWT_SECRET` | Yes | -- | HS256 signing secret (min 32 bytes) |
| `RESEND_API_KEY` | No | -- | Resend transactional email API key |
| `RESEND_FROM` | No | `School Run <onboarding@resend.dev>` | Email sender address |
| `HOST` | No | `0.0.0.0` | Bind address |
| `PORT` | No | `3002` | Bind port |
| `OTP_TTL_SECONDS` | No | `300` | OTP expiry time in seconds |

## Setup

### 1. Run migrations

The auth service requires two migrations against the shared PostgreSQL database:

```bash
# Using sqlx-cli or the engine's migrate command
sr-engine migrate
```

- `008_create_allowed_emails.sql` -- creates the `allowed_emails` table and seeds an initial admin
- `009_create_otp_requests.sql` -- creates the `otp_requests` table with a partial index on active OTPs

### 2. Seed allowed emails

```sql
INSERT INTO allowed_emails (email, role) VALUES ('you@example.com', 'admin');
INSERT INTO allowed_emails (email, role) VALUES ('viewer@example.com', 'viewer');
```

### 3. Start the service

```bash
AUTH_DATABASE_URL=postgres://user:pass@localhost/sr \
JWT_SECRET=your-secret-at-least-32-bytes-long \
cargo run -p sr-auth
```

In development, omit `RESEND_API_KEY` to have OTP codes logged to the console instead of emailed.

---

## Endpoints

### `POST /auth/request-otp`

Request a one-time password for the given email. Always returns the same response regardless of whether the email exists (anti-enumeration).

**Request body:**
```json
{
  "email": "user@example.com"
}
```

**Response:** `200 OK`
```json
{
  "message": "If this email is registered, a code was sent."
}
```

**Errors:**
- `400` -- empty email field
- `429` -- rate limit exceeded

---

### `POST /auth/verify-otp`

Verify an OTP code and receive a JWT on success.

**Request body:**
```json
{
  "email": "user@example.com",
  "otp": "847291"
}
```

**Response:** `200 OK`
```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "expires_at": "2026-03-08T12:00:00+00:00"
}
```

**Errors:**
- `400` -- empty email or OTP field
- `401` -- invalid OTP, expired OTP, no active OTP, or max attempts exceeded
- `429` -- rate limit exceeded

---

### `GET /auth/me`

Returns the authenticated user's email and role from the JWT claims.

**Request headers:**
```
Authorization: Bearer <jwt_token>
```

**Response:** `200 OK`
```json
{
  "email": "user@example.com",
  "role": "viewer"
}
```

**Errors:**
- `401` -- missing, malformed, or expired token

---

### `POST /auth/logout`

Stateless logout. Returns 204 immediately. The client is responsible for removing the JWT from `localStorage`.

**Request headers:**
```
Authorization: Bearer <jwt_token>
```

**Response:** `204 No Content`

---

## Error Format

All errors follow the same JSON structure used by the engine API:

```json
{
  "error": {
    "code": "UNAUTHORIZED",
    "message": "unauthorized: invalid OTP code"
  }
}
```

Error codes: `BAD_REQUEST` (400), `UNAUTHORIZED` (401), `RATE_LIMITED` (429), `INTERNAL_ERROR` (500), `DATABASE_ERROR` (500).

---

## Security

### OTP Properties

| Property | Value | Rationale |
|---|---|---|
| Length | 6 digits (000000--999999) | Standard, user-friendly |
| Generation | `rand::thread_rng()` via `OsRng` | Cryptographically secure |
| Storage | Argon2id hash (never plaintext) | Resistant to database leak |
| TTL | 5 minutes | Short window, reduces replay risk |
| Max attempts | 3 per OTP | 1M combinations, 3 guesses = 0.0003% chance |
| Single use | Consumed on successful verify | Cannot replay |

### Anti-Enumeration

`POST /auth/request-otp` always returns `200` with the message `"If this email is registered, a code was sent."` regardless of whether the email is in the `allowed_emails` table. This prevents attackers from discovering valid email addresses.

### Rate Limits

| Scope | Limit | Window | Response |
|---|---|---|---|
| Per email (request OTP) | 3 requests | 15 minutes | 429 |
| Per IP (request OTP) | 10 requests | 15 minutes | 429 |
| Per email (verify OTP) | 5 attempts | 15 minutes | 429 + invalidate all active OTPs |
| Per IP (verify OTP) | 20 attempts | 15 minutes | 429 |
| Global (request OTP) | 100 requests | 1 minute | 429 (DDoS protection) |

Implementation: In-memory sliding window via `dashmap` + `Instant`. Counts reset naturally as timestamps fall outside the window. No Redis/Valkey dependency.

---

## JWT

| Property | Value |
|---|---|
| Algorithm | HS256 |
| Secret | `JWT_SECRET` env var (min 32 bytes) |
| Expiry | 24 hours |
| Claims | `{ sub: email, role: "viewer" \| "admin", exp, iat }` |
| Refresh | None -- re-authenticate via OTP after 24h |

### Engine API Integration

The engine API validates JWTs using the same `JWT_SECRET` shared between services. Add a `tower` middleware layer that:

1. Extracts the `Authorization: Bearer <token>` header
2. Validates the HS256 signature and expiration
3. Passes decoded claims downstream (email, role)
4. Skips validation for `GET /api/health`

If the token is invalid or expired, return `401 Unauthorized`. The dashboard intercepts 401 responses and redirects to the login page.

---

## Development Mode

When `RESEND_API_KEY` is **not set**, the auth service operates in development mode:

- OTP codes are logged to the console via `tracing::info!` instead of being emailed
- Log output: `DEV MODE OTP for user@example.com: 847291`
- All other behavior (hashing, rate limiting, JWT issuance) remains identical

This allows local development without configuring an email provider.

---

## Database Tables

### `allowed_emails` (migration 008)

```sql
CREATE TABLE allowed_emails (
    id          SERIAL PRIMARY KEY,
    email       TEXT NOT NULL UNIQUE,
    role        TEXT NOT NULL DEFAULT 'viewer',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### `otp_requests` (migration 009)

```sql
CREATE TABLE otp_requests (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT NOT NULL,
    otp_hash        TEXT NOT NULL,
    attempts        SMALLINT NOT NULL DEFAULT 0,
    max_attempts    SMALLINT NOT NULL DEFAULT 3,
    expires_at      TIMESTAMPTZ NOT NULL,
    consumed        BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_otp_requests_email_active
    ON otp_requests (email, expires_at)
    WHERE consumed = FALSE;
```

---

## Crate Structure

```
auth/
  Cargo.toml
  Dockerfile
  src/
    main.rs           # Axum server, env config, startup
    lib.rs            # Public module declarations
    routes.rs         # request_otp, verify_otp, me, logout handlers
    otp.rs            # generate, hash (Argon2id), verify
    jwt.rs            # create_token, validate_token, Claims
    rate_limit.rs     # SlidingWindow RateLimiter via dashmap
    db.rs             # allowed_emails + otp_requests queries (SQLx)
    error.rs          # AuthError enum with IntoResponse
    email.rs          # send_otp_email (Resend API / dev console fallback)
```
