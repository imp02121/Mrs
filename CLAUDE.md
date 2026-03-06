# School Run (sr) - Project Conventions

## Project Overview

School Run is a trading strategy backtester based on Tom Hougaard's School Run Strategy. Rust engine + React dashboard + Telegram bot. See `BUILD.md` for full architecture and build phases.

## Model Requirements

- All AI agents working on this project MUST use **Claude Opus 4.6** (model ID: `claude-opus-4-6`).
- Never use Sonnet, Haiku, or any other model for code generation, review, or documentation.
- All agent work MUST go through the Team system with proper task tracking.

## Repository Structure

```
sr/
  engine/          # Rust: strategy engine, backtester, HTTP API
  dashboard/       # React + Vite + TypeScript: web UI
  telegram/        # Rust: Telegram notification bot
  migrations/      # PostgreSQL migrations (shared)
  data/            # Historical OHLCV data (gitignored)
  docs/            # Strategy docs, API docs, architecture
```

## Rust Conventions (engine/ and telegram/)

### Error Handling
- **Never use `.unwrap()` or `.expect()` in production code.** Use `?` with proper error types.
- Use `thiserror` for library error types, `anyhow` only in `main.rs` and CLI entry points.
- Define domain-specific error enums per module (e.g., `StrategyError`, `DataError`, `ApiError`).
- All errors must be actionable -- include context about what went wrong and why.

```rust
// GOOD
#[derive(Debug, thiserror::Error)]
pub enum StrategyError {
    #[error("no signal bar found for {instrument} on {date}")]
    NoSignalBar { instrument: String, date: NaiveDate },
    #[error("invalid stop loss mode: {0}")]
    InvalidStopLoss(String),
}

// BAD
let value = map.get("key").unwrap(); // NEVER in production
```

### Code Style
- Run `cargo fmt` before every commit. CI will reject unformatted code.
- Run `cargo clippy -- -D warnings` before every commit. Zero warnings allowed.
- Use `rust_decimal::Decimal` for all financial calculations, never raw `f64`. Exception: `f64` is acceptable for statistical ratios (Sharpe, win_rate) where exact precision is not critical.
- Prefer `&str` over `String` in function parameters where ownership is not needed.
- Use `#[must_use]` on functions that return values that should not be ignored.
- Avoid `clone()` unless necessary. Prefer references and borrowing.

### Naming
- Modules: `snake_case` (e.g., `signal_bar.rs`, `add_to_winners.rs`)
- Types: `PascalCase` (e.g., `SignalBar`, `BacktestResult`)
- Functions: `snake_case` (e.g., `find_signal_bar`, `compute_stop_loss`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `DEFAULT_ENTRY_OFFSET`)
- Enum variants: `PascalCase` (e.g., `StopLossMode::FixedPoints`)

### Documentation
- All `pub` functions, structs, and enums MUST have doc comments (`///`).
- Doc comments should explain **what** and **why**, not just restate the name.
- Include `# Examples` in doc comments for complex functions.
- Module-level docs (`//!`) at the top of each file explaining the module's purpose.

```rust
/// Identifies the signal bar for a given trading day and instrument.
///
/// The signal bar is the Nth 15-minute candle after market open (default: 2nd candle).
/// For DAX, this is the 09:15-09:30 CET candle.
///
/// Returns `None` if the signal bar candle is missing (holiday, data gap).
///
/// # Arguments
/// * `candles` - Day's candles for the instrument, sorted by timestamp
/// * `instrument` - The trading instrument (determines market open time)
/// * `date` - The trading date
/// * `config` - Strategy configuration (determines bar index and offset)
pub fn find_signal_bar(
    candles: &[Candle],
    instrument: Instrument,
    date: NaiveDate,
    config: &StrategyConfig,
) -> Option<SignalBar> {
```

### Testing
- **Every module MUST have a `#[cfg(test)] mod tests` block.**
- **Every `pub` function MUST have at least one unit test.**
- Use descriptive test names: `test_signal_bar_found_for_dax_winter_date`, not `test_1`.
- Strategy logic tests must be pure (no I/O, no async) -- test with constructed candle data.
- Use `insta` crate for snapshot testing of complex output (BacktestResult, Trade serialization).
- Use `criterion` for benchmarks of performance-critical paths (backtest loop, parameter sweep).
- Integration tests go in `tests/` directory, require `--features integration` flag.
- **Target: >80% code coverage on strategy and backtest modules.**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_bar_found_for_dax_winter_date() {
        let candles = make_test_candles_dax_winter();
        let result = find_signal_bar(&candles, Instrument::Dax, date(2024, 1, 15), &default_config());
        assert!(result.is_some());
        let bar = result.unwrap();
        assert_eq!(bar.candle.timestamp.hour(), 8); // 08:15 UTC = 09:15 CET in winter
        assert_eq!(bar.candle.timestamp.minute(), 15);
    }

    #[test]
    fn test_signal_bar_not_found_on_holiday() {
        let candles = vec![]; // no candles on holiday
        let result = find_signal_bar(&candles, Instrument::Dax, date(2024, 12, 25), &default_config());
        assert!(result.is_none());
    }
}
```

### Test Helpers
- Create a `engine/src/test_helpers.rs` module (compiled only in test) with:
  - `make_candle(timestamp, open, high, low, close)` -- factory for test candles
  - `make_day_candles(instrument, date, signal_bar_ohlc, post_bar_candles)` -- builds a full day
  - `default_config()` -- returns a reasonable default `StrategyConfig`
  - `date(y, m, d)` -- shorthand for `NaiveDate::from_ymd_opt(y, m, d).unwrap()`

### Dependencies
- Pin major versions in `Cargo.toml`. Use workspace dependencies.
- Justify every new dependency in the PR/commit message.
- Prefer well-maintained crates with >1000 downloads/week.
- Check for `unsafe` code in new dependencies.

## TypeScript/React Conventions (dashboard/)

### Code Style
- Run `npx prettier --write .` before every commit.
- Run `npx eslint .` before every commit. Zero warnings.
- Use strict TypeScript (`"strict": true` in tsconfig).
- No `any` types. Ever. Use `unknown` and narrow with type guards.
- Prefer `interface` over `type` for object shapes.
- Use `const` by default. Only `let` when mutation is needed. Never `var`.

### Components
- Functional components only. No class components.
- One component per file. File name matches component name (PascalCase).
- Props interfaces defined in the same file, named `{ComponentName}Props`.
- Use React hooks. No HOCs.

### API Types
- Mirror Rust types exactly in `src/types/`. When a Rust struct changes, the TS type must change.
- API response types must match the JSON shape from the engine.
- Use Zod for runtime validation of API responses in development.

### Testing
- Use Vitest for unit tests.
- Every component must have a test file (`ComponentName.test.tsx`).
- Test user interactions, not implementation details.
- Use MSW (Mock Service Worker) for API mocking in tests.

## Git Conventions

### Commits
- Use conventional commits: `feat:`, `fix:`, `test:`, `docs:`, `refactor:`, `ci:`, `chore:`
- Commit message format: `type(scope): description`
  - `feat(engine): implement signal bar detection for DAX`
  - `test(engine): add unit tests for stop loss modes`
  - `docs(api): document backtest endpoints`
  - `fix(dashboard): correct equity curve rendering with negative values`
- Keep commits atomic -- one logical change per commit.
- Never commit `.env` files, API keys, or secrets.

### Branches
- `main` -- stable, tested, documented
- `feat/phase-{n}-{description}` -- feature branches per phase
- `fix/{description}` -- bug fixes
- `docs/{description}` -- documentation only

## CI/CD (GitHub Actions)

### On every push/PR:
1. `cargo fmt --check` -- formatting
2. `cargo clippy -- -D warnings` -- linting
3. `cargo test` -- unit tests
4. `cd dashboard && npm run lint` -- TS linting
5. `cd dashboard && npm run typecheck` -- TS type checking
6. `cd dashboard && npm run test` -- Vitest

### On merge to main:
7. `cargo test --features integration` -- integration tests (requires Postgres + Valkey)
8. Docker image builds for all three services

## Quality Gates

A task is NOT complete until:
1. Implementation code is written
2. Unit tests pass (every pub function tested)
3. `cargo fmt` and `cargo clippy -- -D warnings` pass (Rust) or `prettier` and `eslint` pass (TS)
4. Doc comments on all pub items
5. PM (team lead) has reviewed the code
6. BUILD.md updated if the phase is complete

## Data Conventions

- All timestamps stored as UTC internally.
- Convert to exchange local time only at display boundaries (dashboard, Telegram messages).
- Use `chrono-tz` for timezone conversions. Account for DST.
- Financial values use `Decimal`. Statistical ratios use `f64`.
- Candle data files are gitignored. Document how to obtain them in `data/README.md`.

## API Conventions

- All endpoints under `/api/` prefix.
- JSON request and response bodies.
- Consistent error format: `{ "error": { "code": "...", "message": "...", "details": ... } }`
- Pagination: `?page=1&per_page=50` with wrapper `{ "data": [...], "pagination": {...} }`
- Use appropriate HTTP status codes (200, 201, 204, 400, 404, 422, 500).
- CORS enabled for dashboard origin.
