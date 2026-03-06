//! HTTP REST API built with Axum.
//!
//! ## Architecture
//!
//! [`AppState`] holds a `PgPool` and optional `ValkeyCache`, shared across
//! all handlers via Axum's state extractor. [`api_routes`] assembles the
//! full router with middleware (CORS, tracing, timeout, compression).
//!
//! ## Endpoint groups
//!
//! - **Health** (`/api/health`) — liveness probe
//! - **Backtest** (`/api/backtest/*`) — run backtests, fetch results, compare configs, history
//! - **Configs** (`/api/configs/*`) — CRUD for strategy configurations
//! - **Data** (`/api/data/*`) — instruments list, candle queries, data fetch trigger
//! - **Signals** (`/api/signals/*`) — today's signals, latest signal per instrument
//!
//! All responses use [`ApiResponse`] or [`PaginatedResponse`] wrappers.
//! Errors are returned as [`ApiError`] with structured JSON bodies.

pub mod backtest;
pub mod configs;
pub mod data;
pub mod error;
pub mod response;
pub mod router;
pub mod signals;
pub mod state;

pub use error::ApiError;
pub use response::{ApiResponse, PaginatedResponse, Pagination, PaginationParams};
pub use router::api_routes;
pub use state::AppState;
