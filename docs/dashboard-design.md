# Dashboard & Auth Design Document

> Phase 6 planning: authentication microservice + React dashboard wireframes.
> This document must be reviewed and approved before any code is written.

---

## Part 1: Authentication Microservice

### Overview

A standalone Rust microservice (`auth/`) that gates access to the dashboard. No signup flow -- only pre-approved email addresses in PostgreSQL can log in. Authentication is OTP-only (no passwords), with aggressive rate limiting and encrypted OTP storage.

### Architecture

```
Browser                  Auth Service (Rust/Axum)              PostgreSQL
  |                            |                                  |
  |  POST /auth/request-otp   |                                  |
  |  { email }                |                                  |
  |--------------------------->|                                  |
  |                            |  SELECT FROM allowed_emails      |
  |                            |--------------------------------->|
  |                            |  (exists? rate limit ok?)        |
  |                            |<---------------------------------|
  |                            |                                  |
  |                            |  Generate OTP (6 digits)         |
  |                            |  Hash with Argon2                |
  |                            |  Store hash + expires_at in DB   |
  |                            |  Send email via SMTP             |
  |                            |                                  |
  |  200 { message }          |                                  |
  |<---------------------------|                                  |
  |                            |                                  |
  |  POST /auth/verify-otp    |                                  |
  |  { email, otp }           |                                  |
  |--------------------------->|                                  |
  |                            |  Fetch OTP record for email      |
  |                            |  Verify: Argon2, TTL, attempts   |
  |                            |  Issue JWT (HS256, 24h expiry)   |
  |                            |                                  |
  |  200 { token, expires_at }|                                  |
  |<---------------------------|                                  |
  |                            |                                  |
  |  GET /auth/me              |                                  |
  |  Authorization: Bearer ... |                                  |
  |--------------------------->|                                  |
  |  200 { email, role }      |                                  |
  |<---------------------------|                                  |
```

### Database Tables (new migrations)

```sql
-- 008_create_allowed_emails.sql
CREATE TABLE allowed_emails (
    id          SERIAL PRIMARY KEY,
    email       TEXT NOT NULL UNIQUE,
    role        TEXT NOT NULL DEFAULT 'viewer',  -- 'viewer' | 'admin'
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed with initial admin
INSERT INTO allowed_emails (email, role) VALUES ('admin@example.com', 'admin');

-- 009_create_otp_requests.sql
CREATE TABLE otp_requests (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT NOT NULL,
    otp_hash        TEXT NOT NULL,           -- Argon2 hash
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

### OTP Security

| Property | Value | Rationale |
|---|---|---|
| OTP length | 6 digits (000000-999999) | Standard, user-friendly |
| Generation | `rand::thread_rng()` via `OsRng` | Cryptographically secure |
| Storage | Argon2id hash (never plaintext) | Resistant to DB leak |
| TTL | 5 minutes | Short window, reduces replay risk |
| Max attempts | 3 per OTP | Prevents brute force (1M combos, 3 guesses = 0.0003%) |
| Consumed on verify | Yes | Single use, cannot replay |
| Cleanup | Cron or ON INSERT trigger deletes expired rows | No stale data |

### Rate Limiting

| Scope | Limit | Window | Response |
|---|---|---|---|
| Per email (request OTP) | 3 requests | 15 minutes | 429 "Too many requests. Try again in X minutes." |
| Per IP (request OTP) | 10 requests | 15 minutes | 429 |
| Per email (verify OTP) | 5 attempts | 15 minutes | 429 + invalidate all active OTPs for email |
| Per IP (verify OTP) | 20 attempts | 15 minutes | 429 |
| Global | 100 OTP requests | 1 minute | 429 (DDoS protection) |

Implementation: In-memory sliding window via `dashmap` + `Instant`. Counts reset naturally. No Redis dependency needed for auth service.

### JWT

| Property | Value |
|---|---|
| Algorithm | HS256 |
| Secret | `JWT_SECRET` env var (min 32 bytes) |
| Expiry | 24 hours |
| Claims | `{ sub: email, role: "viewer"|"admin", exp, iat }` |
| Refresh | None. Re-authenticate via OTP after 24h. |

### Email Delivery

- Use **Resend** transactional email API via `reqwest`
- Env var: `RESEND_API_KEY`
- From address: `onboarding@resend.dev` (default) or custom domain
- Plain text email: "Your School Run login code: 847291. Expires in 5 minutes."
- In development: if `RESEND_API_KEY` is unset, log OTP to console instead of sending email
- Single POST to `https://api.resend.com/emails` -- no SMTP config needed

### Auth Service Endpoints

| Method | Path | Body | Response | Auth |
|---|---|---|---|---|
| `POST` | `/auth/request-otp` | `{ "email": "..." }` | `200 { "message": "If this email is registered, a code was sent." }` | None |
| `POST` | `/auth/verify-otp` | `{ "email": "...", "otp": "123456" }` | `200 { "token": "jwt...", "expires_at": "..." }` | None |
| `GET` | `/auth/me` | -- | `200 { "email": "...", "role": "..." }` | Bearer JWT |
| `POST` | `/auth/logout` | -- | `204` | Bearer JWT |

Note: `request-otp` always returns 200 with the same message regardless of whether the email exists. This prevents email enumeration.

### Auth Service Config

```
AUTH_DATABASE_URL=postgres://...    # Same DB, or separate
JWT_SECRET=<min-32-byte-secret>
OTP_TTL_SECONDS=300
RESEND_API_KEY=re_...              # Resend transactional email API key
RESEND_FROM=onboarding@resend.dev  # Or custom domain sender
HOST=0.0.0.0
PORT=3002
```

### Crate Structure

```
auth/
  Cargo.toml
  Dockerfile
  src/
    main.rs           # Axum server, env config, routes
    routes.rs         # request_otp, verify_otp, me, logout
    otp.rs            # generate, hash, verify
    jwt.rs            # create_token, validate_token, Claims
    rate_limit.rs     # SlidingWindow, RateLimiter middleware
    db.rs             # allowed_emails queries, otp_requests queries
    error.rs          # AuthError enum
    email.rs          # send_otp_email (Resend API via reqwest)
```

### Dashboard Integration

The dashboard stores the JWT in `localStorage`. Every API request includes `Authorization: Bearer <token>`. The engine API validates the JWT using the same `JWT_SECRET`. If invalid/expired, return 401 -> dashboard redirects to login page.

Engine API needs a thin JWT validation middleware (extract + verify), not the full auth service. Add `tower` middleware layer that checks the `Authorization` header on all routes except `/api/health`.

---

## Part 2: Dashboard Design

### Design Principles

- **White, clean theme** -- not a dark trading terminal. Think Notion/Linear, not Bloomberg.
- **Informative but uncluttered** -- show the data that matters, hide complexity behind expandable sections.
- **Consistent layout** -- fixed sidebar nav, content area with consistent padding and card-based layout.
- **Typography-first** -- data is primarily numbers; use a good monospace for values, clean sans-serif for labels.
- **Responsive** -- works on 1280px+ screens. Not mobile-optimized (desktop tool).

### Color Palette

```
Background:    #FFFFFF (white)
Surface:       #F9FAFB (gray-50, cards)
Border:        #E5E7EB (gray-200)
Text primary:  #111827 (gray-900)
Text secondary:#6B7280 (gray-500)
Accent:        #2563EB (blue-600, primary actions)
Accent hover:  #1D4ED8 (blue-700)
Success/Profit:#059669 (emerald-600)
Loss/Danger:   #DC2626 (red-600)
Warning:       #D97706 (amber-600)
```

### Typography

```
Headings:      Inter (or system sans-serif), font-semibold
Body:          Inter, font-normal
Data values:   JetBrains Mono (monospace), tabular-nums
```

### Layout

```
+----------------------------------------------------------+
|  School Run          [instrument selector]    [user menu] |
+--------+-------------------------------------------------+
|        |                                                  |
|  NAV   |              CONTENT AREA                        |
|        |                                                  |
| Backtest|             Page-specific content               |
| Compare |             rendered here                       |
| History |                                                 |
| Signals |                                                 |
| Data    |                                                 |
|         |                                                 |
|         |                                                 |
+---------+-------------------------------------------------+
```

- **Sidebar**: 220px wide, fixed. Logo at top, nav links below. Active link highlighted with blue-600 left border + blue-50 background.
- **Top bar**: 56px height. Global instrument selector (dropdown), user avatar/email, logout.
- **Content**: max-width 1400px, centered, 24px padding.

---

### Page Wireframes

#### 1. Login Page (`/login`)

No sidebar. Centered card on a light gray background.

```
+----------------------------------------------------------+
|                                                          |
|                                                          |
|              +----------------------------+              |
|              |                            |              |
|              |      School Run            |              |
|              |      Trading Backtester    |              |
|              |                            |              |
|              |  Email                     |              |
|              |  [________________________]|              |
|              |                            |              |
|              |  [  Send login code  ]     |              |
|              |                            |              |
|              +----------------------------+              |
|                                                          |
+----------------------------------------------------------+
```

After clicking "Send login code":

```
|              +----------------------------+              |
|              |                            |              |
|              |      Enter your code       |              |
|              |                            |              |
|              |  We sent a 6-digit code    |              |
|              |  to j***@example.com       |              |
|              |                            |              |
|              |  Code                      |              |
|              |  [__ __ __ __ __ __]       |              |
|              |                            |              |
|              |  [    Verify     ]         |              |
|              |                            |              |
|              |  Didn't get it? Resend     |              |
|              |  (available in 60s)        |              |
|              |                            |              |
|              +----------------------------+              |
```

- 6-digit input with auto-advance between boxes
- Resend cooldown: 60 seconds
- After 3 failed attempts: "Too many attempts. Please request a new code."
- On success: redirect to `/` (Backtest page)

---

#### 2. Backtest Page (`/`) -- Default landing

Two-column layout: config panel on left (360px), results on right.

```
+--------+--------------------------------------------+
| NAV    |  BACKTEST                                  |
|        |                                            |
|        |  +-------------+  +---------------------+  |
|        |  | CONFIG      |  | RESULTS             |  |
|        |  |             |  |                     |  |
|        |  | Instrument  |  | Stats Cards Row:    |  |
|        |  | [DAX    v]  |  | +------+ +------+   |  |
|        |  |             |  | |Trades| |WinRate|  |  |
|        |  | Date Range  |  | | 247  | | 54.3%|  |  |
|        |  | [2024-01-01]|  | +------+ +------+   |  |
|        |  | [2024-12-31]|  | +------+ +------+   |  |
|        |  |             |  | |Profit| |Sharpe|   |  |
|        |  | Stop Loss   |  | |Factor| | 1.42 |   |  |
|        |  | Mode [v]    |  | | 1.87 | +------+   |  |
|        |  | Points [40] |  | +------+            |  |
|        |  |             |  |                     |  |
|        |  | Exit Mode   |  | Equity Curve        |  |
|        |  | [EndOfDay v]|  | [=====chart======]  |  |
|        |  |             |  |                     |  |
|        |  | Add to Win  |  | Monthly PnL Heatmap |  |
|        |  | [ ] Enabled |  | [=====grid=======]  |  |
|        |  | Every [50]  |  |                     |  |
|        |  | Max [3]     |  | Drawdown Chart      |  |
|        |  |             |  | [=====chart======]  |  |
|        |  | Advanced ^  |  |                     |  |
|        |  | (collapsed) |  | Trade Table         |  |
|        |  |             |  | # Dir Entry Exit PnL|  |
|        |  | [Run]       |  | 1 L   16800 16850 50|  |
|        |  +-------------+  | 2 S   16750 16700 50|  |
|        |                   | ...                 |  |
|        |                   +---------------------+  |
+--------+--------------------------------------------+
```

**Config Panel sections (collapsible):**
1. **Instrument & Dates** -- always visible
2. **Stop Loss** -- mode dropdown, points input, scale toggle
3. **Exit Strategy** -- mode dropdown, conditional fields
4. **Adding to Winners** -- enable toggle, conditional fields
5. **Advanced** -- collapsed by default: signal bar index, entry offset, slippage, commission

**Results section (shown after a backtest completes):**
1. **Stats cards** -- 2x3 grid of key metrics:
   - Total trades | Win rate
   - Profit factor | Sharpe ratio
   - Max drawdown | Net PnL (points)
2. **Equity curve** -- Recharts line chart, cumulative PnL over time
3. **Monthly PnL heatmap** -- grid: months as columns, years as rows, colored cells (green = profit, red = loss)
4. **Drawdown chart** -- Recharts area chart (inverted, red fill)
5. **Trade distribution** -- histogram of PnL per trade
6. **Trade table** -- TanStack Table with columns: #, Date, Dir, Instrument, Entry, Exit, SL, PnL, Duration, Adds. Sortable, paginated (25/page).

**Loading state**: Skeleton cards + progress shimmer during backtest execution.

---

#### 3. Compare Page (`/compare`)

Side-by-side comparison of 2-4 configurations.

```
+--------+--------------------------------------------+
| NAV    |  COMPARE BACKTESTS                         |
|        |                                            |
|        |  Config Slots:                             |
|        |  +----------+ +----------+ [+ Add Config] |
|        |  | Config A | | Config B |                |
|        |  | DAX      | | DAX      |                |
|        |  | SL: 40   | | SL: 60   |                |
|        |  | EOD      | | Trailing |                |
|        |  | [Edit]   | | [Edit]   |                |
|        |  +----------+ +----------+                |
|        |                                            |
|        |  [    Run Comparison    ]                  |
|        |                                            |
|        |  Comparison Table:                         |
|        |  +------------------------------------+    |
|        |  | Metric     | Config A | Config B  |    |
|        |  |------------|----------|-----------|    |
|        |  | Trades     | 247      | 312       |    |
|        |  | Win Rate   | 54.3%    | 48.7%     |    |
|        |  | PF         | 1.87     | 1.32      |    |
|        |  | Sharpe     | 1.42     | 1.05      |    |
|        |  | Max DD     | -2,400   | -3,100    |    |
|        |  | Net PnL    | +12,400  | +8,200    |    |
|        |  +------------------------------------+    |
|        |                                            |
|        |  Overlaid Equity Curves:                   |
|        |  [======= chart with 2 lines ========]    |
|        |                                            |
|        |  Monthly PnL side-by-side:                 |
|        |  [Config A heatmap] [Config B heatmap]    |
|        |                                            |
+--------+--------------------------------------------+
```

- Click "Edit" on a config slot -> opens a modal with the same config panel from Backtest page
- Best value in each metric row highlighted in bold/blue
- Equity curves overlaid on same chart with different colors
- Max 4 configs (as enforced by API)

---

#### 4. History Page (`/history`)

Browse past backtest runs.

```
+--------+--------------------------------------------+
| NAV    |  BACKTEST HISTORY                          |
|        |                                            |
|        |  Filters:                                  |
|        |  Instrument [All v]  Date [______] - [___] |
|        |                                            |
|        |  +----------------------------------------+|
|        |  | Run        | Instrument | Dates        ||
|        |  | ID         |            | Trades | PnL ||
|        |  |------------|------------|--------|-----||
|        |  | 7882ad1... | DAX        | Jan-Dec||
|        |  |            |            |  247   |+12k ||
|        |  |------------|------------|--------|-----||
|        |  | a3f29b4... | FTSE       | Jan-Jun||
|        |  |            |            |  128   |+4.2k||
|        |  +----------------------------------------+|
|        |                                            |
|        |  < 1 2 3 ... 7 >  (pagination)            |
|        |                                            |
+--------+--------------------------------------------+
```

- Click a row -> navigates to `/backtest/:id` showing full results (same layout as Backtest page results section, but read-only)
- Sortable columns
- Pagination via API (`?page=0&per_page=25`)
- Optional: delete button (admin only)

---

#### 5. Signals Page (`/signals`)

Today's live signals + latest signal per instrument.

```
+--------+--------------------------------------------+
| NAV    |  TODAY'S SIGNALS          March 7, 2026    |
|        |                                            |
|        |  +------------------+ +------------------+ |
|        |  | DAX              | | FTSE             | |
|        |  | Signal Bar:      | | Signal Bar:      | |
|        |  | 09:15-09:30 CET  | | 08:15-08:30 GMT  | |
|        |  |                  | |                  | |
|        |  | High: 18,450.50  | | High: 8,245.25   | |
|        |  | Low:  18,392.00  | | Low:  8,218.50   | |
|        |  |                  | |                  | |
|        |  | Buy:  18,452.50  | | Buy:  8,247.25   | |
|        |  | Sell: 18,390.00  | | Sell: 8,216.50   | |
|        |  |                  | |                  | |
|        |  | Status: PENDING  | | Status: FILLED   | |
|        |  |         (amber)  | |    BUY (green)   | |
|        |  +------------------+ +------------------+ |
|        |                                            |
|        |  +------------------+ +------------------+ |
|        |  | NASDAQ           | | DOW              | |
|        |  | ...              | | ...              | |
|        |  +------------------+ +------------------+ |
|        |                                            |
+--------+--------------------------------------------+
```

- 2x2 grid of signal cards, one per instrument
- Status badge: PENDING (amber), FILLED (green), EXPIRED (gray), NO SIGNAL (muted)
- If no signal yet today (market hasn't opened): "Awaiting signal bar" message
- Auto-refresh every 60 seconds via TanStack Query `refetchInterval`

---

#### 6. Data Page (`/data`)

View instrument metadata and candle data coverage.

```
+--------+--------------------------------------------+
| NAV    |  DATA                                      |
|        |                                            |
|        |  Instruments:                              |
|        |  +----------------------------------------+|
|        |  | Symbol | Name     | Open  | Close |    ||
|        |  |--------|----------|-------|-------|    ||
|        |  | DAX    | DAX 40   | 09:00 | 17:30 |    ||
|        |  | FTSE   | FTSE 100 | 08:00 | 16:30 |    ||
|        |  | IXIC   | Nasdaq   | 09:30 | 16:00 |    ||
|        |  | DJI    | Dow 30   | 09:30 | 16:00 |    ||
|        |  +----------------------------------------+|
|        |                                            |
|        |  Candle Explorer:                          |
|        |  Instrument [DAX v] From [___] To [___]    |
|        |  [  Load Candles  ]                        |
|        |                                            |
|        |  Showing 340 candles for DAX (2024-01-02   |
|        |  to 2024-01-31)                            |
|        |                                            |
|        |  [====== candlestick chart =========]      |
|        |                                            |
|        |  +----------------------------------------+|
|        |  | Timestamp        | O     | H     | ... ||
|        |  |------------------|-------|-------|-----||
|        |  | 2024-01-02 08:15 |16800.5|16825.0| ... ||
|        |  +----------------------------------------+|
|        |                                            |
+--------+--------------------------------------------+
```

- Instruments table: read-only from API
- Candle explorer: select instrument + date range, load data, display as Lightweight Charts candlestick + raw data table below
- Future: "Fetch Data" button to trigger data download from provider

---

### Page Summary

| Route | Page | Purpose |
|---|---|---|
| `/login` | Login | OTP email authentication |
| `/` | Backtest | Run backtests with config panel + view results |
| `/backtest/:id` | Backtest Detail | View saved backtest result (read-only) |
| `/compare` | Compare | Side-by-side 2-4 config comparison |
| `/history` | History | Browse past backtest runs |
| `/signals` | Signals | Today's signal levels, all instruments |
| `/data` | Data | Instruments list, candle explorer |

### Component Tree

```
App
 +-- AuthProvider (context: user, token, login/logout)
 |
 +-- (unauthenticated)
 |   +-- LoginPage
 |       +-- EmailForm
 |       +-- OtpForm
 |
 +-- (authenticated)
     +-- AppShell
         +-- Sidebar
         |   +-- NavLink (x6)
         +-- TopBar
         |   +-- InstrumentSelector
         |   +-- UserMenu
         +-- <Outlet /> (react-router)
             +-- BacktestPage
             |   +-- ConfigPanel
             |   |   +-- InstrumentDateSection
             |   |   +-- StopLossSection
             |   |   +-- ExitSection
             |   |   +-- AddToWinnersSection
             |   |   +-- AdvancedSection
             |   +-- ResultsPanel
             |       +-- StatsCards
             |       +-- EquityCurve (Recharts)
             |       +-- MonthlyHeatmap
             |       +-- DrawdownChart (Recharts)
             |       +-- TradeDistribution (Recharts)
             |       +-- TradeTable (TanStack Table)
             +-- ComparePage
             |   +-- ConfigSlot (x2-4)
             |   +-- ComparisonTable
             |   +-- OverlaidEquityCurves
             +-- HistoryPage
             |   +-- FilterBar
             |   +-- RunsTable (TanStack Table)
             |   +-- Pagination
             +-- BacktestDetailPage
             |   +-- ResultsPanel (same as BacktestPage, read-only)
             +-- SignalsPage
             |   +-- SignalCard (x4)
             +-- DataPage
                 +-- InstrumentsTable
                 +-- CandleExplorer
                     +-- CandlestickChart (Lightweight Charts)
                     +-- CandleTable
```

### State Management

| Store | Library | Contents |
|---|---|---|
| Auth | React Context | `user`, `token`, `login()`, `logout()`, `isAuthenticated` |
| Server state | TanStack Query | All API data (backtests, configs, signals, candles, instruments) |
| Backtest config | Zustand | Current form values in config panel (persists across navigations) |
| UI | Zustand | Sidebar collapsed state |

### API Client

```typescript
// api/client.ts
const api = axios.create({
  baseURL: import.meta.env.VITE_API_URL ?? "http://localhost:3001/api",
  headers: { "Content-Type": "application/json" },
});

// Request interceptor: attach JWT
api.interceptors.request.use((config) => {
  const token = localStorage.getItem("sr_token");
  if (token) config.headers.Authorization = `Bearer ${token}`;
  return config;
});

// Response interceptor: 401 -> redirect to /login
api.interceptors.response.use(
  (res) => res,
  (err) => {
    if (err.response?.status === 401) {
      localStorage.removeItem("sr_token");
      window.location.href = "/login";
    }
    return Promise.reject(err);
  }
);
```

### New Dependencies Needed

**Dashboard:**
- None beyond what's already in package.json

**Auth service (new Cargo.toml):**
- `axum`, `tokio`, `serde`, `serde_json` (workspace deps)
- `sqlx` (workspace dep)
- `reqwest` (workspace dep) -- Resend API calls
- `jsonwebtoken` -- JWT encode/decode
- `argon2` -- OTP hashing
- `dashmap` -- concurrent rate limit maps
- `rand` -- cryptographic OTP generation

---

## Part 3: Build Phases

### Phase 6a: Auth Microservice

1. Add `auth/` crate to workspace
2. Migration 008 (allowed_emails) + 009 (otp_requests)
3. OTP generation + Argon2 hashing
4. Rate limiting middleware
5. JWT create/validate
6. Email sending (lettre + dev console fallback)
7. Axum routes: request-otp, verify-otp, me, logout
8. JWT validation middleware for engine API
9. Tests (unit tests for OTP, JWT, rate limiter; no integration tests)

### Phase 6b: Dashboard

1. Auth context + login page + protected routes
2. AppShell layout (sidebar + topbar)
3. API client with JWT interceptor
4. TypeScript types mirroring Rust API
5. TanStack Query hooks for all endpoints
6. Backtest page (config panel + results)
7. Charts (equity curve, drawdown, monthly heatmap, trade distribution)
8. Trade table with sorting + pagination
9. Compare page
10. History page
11. Signals page
12. Data page
13. Tests (Vitest + MSW mocks)

---

## Decisions (Locked In)

1. **Auth service: separate microservice.** Own crate (`auth/`), own port (3002), own Dockerfile. Never slows down the engine.
2. **Email provider: Resend.** Transactional email API via `reqwest`. Simple, no SMTP config. Dev fallback: log OTP to console.
3. **Admin panel: direct DB only.** For MVP, manage `allowed_emails` via SQL. No admin UI.
4. **Session: 24h JWT, no refresh.** Re-authenticate via OTP after expiry. Acceptable for a daytime trading tool.
