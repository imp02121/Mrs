# Contributing to School Run

Thank you for your interest in contributing to School Run! This guide covers the development setup, workflow, and conventions for the project.

## Development Setup

### Prerequisites

| Tool | Version | Purpose |
|---|---|---|
| Rust | stable (latest) | Engine, auth, and telegram services |
| Node.js | 22+ | Dashboard |
| PostgreSQL | 16 | Database |
| Valkey | 8 | Cache (optional for development) |

### Clone and build

```bash
git clone https://github.com/<owner>/sr.git
cd sr

# Build Rust workspace
cargo build --workspace

# Build dashboard
cd dashboard
npm install
npm run dev
```

### Database setup

Start PostgreSQL and create the database:

```bash
createdb school_run

# Run migrations
export DATABASE_URL=postgres://localhost/school_run
cargo run -p sr-engine -- migrate
```

### Running locally

```bash
# Engine (port 3001)
DATABASE_URL=postgres://localhost/school_run cargo run -p sr-engine -- serve

# Auth (port 3002)
AUTH_DATABASE_URL=postgres://localhost/school_run \
JWT_SECRET=dev-secret-at-least-32-bytes-long \
cargo run -p sr-auth

# Dashboard (port 5173)
cd dashboard && npm run dev

# Telegram bot (optional)
TELEGRAM_BOT_TOKEN=your_token \
DATABASE_URL=postgres://localhost/school_run \
cargo run -p sr-telegram
```

## Running Tests

```bash
# Rust unit tests (all crates)
cargo test --workspace

# Dashboard tests
cd dashboard && npm test

# Integration tests (requires running PostgreSQL and Valkey)
cargo test --workspace --features integration
```

## Code Style

### Rust

```bash
# Format code
cargo fmt --all

# Lint (zero warnings required)
cargo clippy --workspace -- -D warnings
```

- Use `rust_decimal::Decimal` for financial values, never `f64`.
- No `.unwrap()` or `.expect()` in production code. Use `?` with proper error types.
- Doc comments (`///`) on all `pub` items.
- Unit tests in every module (`#[cfg(test)] mod tests`).

### TypeScript / React

```bash
cd dashboard

# Format
npx prettier --write .

# Lint (zero warnings required)
npx eslint .

# Type check
npx tsc --noEmit
```

- Strict TypeScript (`"strict": true`). No `any` types.
- Functional components only. One component per file.
- `const` by default, `let` only when mutation is needed.

See [CLAUDE.md](CLAUDE.md) for the complete coding conventions.

## Commit Messages

Use [conventional commits](https://www.conventionalcommits.org/):

```
type(scope): description
```

**Types:** `feat`, `fix`, `test`, `docs`, `refactor`, `ci`, `chore`

**Examples:**

```
feat(engine): implement trailing stop exit mode
fix(dashboard): correct equity curve rendering with negative values
test(engine): add unit tests for DST timezone transitions
docs(api): document backtest comparison endpoint
refactor(auth): extract rate limiter into separate module
```

Keep commits atomic -- one logical change per commit.

## Pull Requests

1. **Fork** the repository and clone your fork.
2. **Branch** from `main`:
   ```bash
   git checkout -b feat/your-feature
   ```
3. **Implement** your changes following the project conventions.
4. **Test** your changes:
   ```bash
   cargo fmt --all --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   cd dashboard && npm run lint && npm run typecheck && npm test
   ```
5. **Commit** with conventional commit messages.
6. **Push** to your fork and open a pull request against `main`.
7. **Describe** what your PR does and why. Link related issues.

### Branch naming

- `feat/{description}` -- new features
- `fix/{description}` -- bug fixes
- `docs/{description}` -- documentation only
- `refactor/{description}` -- code restructuring

## Project Structure

```
sr/
  engine/          # Rust: strategy engine, backtester, HTTP API (Axum)
  auth/            # Rust: OTP authentication microservice (Axum)
  dashboard/       # React + Vite + TypeScript: web UI
  telegram/        # Rust: Telegram notification bot (Teloxide)
  migrations/      # PostgreSQL migrations (shared across services)
  data/            # Historical OHLCV data (gitignored)
  docs/            # Strategy docs, API docs, architecture
```

Key documentation:

- [BUILD.md](BUILD.md) -- architecture and build phases
- [CLAUDE.md](CLAUDE.md) -- detailed coding conventions
- [docs/api.md](docs/api.md) -- HTTP API reference
- [docs/auth.md](docs/auth.md) -- auth service reference
- [docs/strategy.md](docs/strategy.md) -- strategy explanation

## Where to Start

- Check open [issues](https://github.com/<owner>/sr/issues) for tasks.
- Look for issues labeled `good first issue` for beginner-friendly work.
- Read through [docs/strategy.md](docs/strategy.md) to understand the trading strategy.
- Browse the test suites to understand expected behavior.

## Code of Conduct

Be respectful and constructive in all interactions. We are committed to providing a welcoming, inclusive environment for everyone. Harassment, discrimination, and disruptive behavior are not tolerated.
