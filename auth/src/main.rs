//! School Run authentication service entry point.
//!
//! Starts an Axum HTTP server that handles OTP-based authentication
//! for the School Run dashboard. See `docs/dashboard-design.md` for
//! the full authentication specification.

use std::sync::Arc;

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

use sr_auth::rate_limit::RateLimiter;
use sr_auth::routes::auth_routes;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present (development only)
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Read configuration from environment
    let database_url =
        std::env::var("AUTH_DATABASE_URL").context("AUTH_DATABASE_URL must be set")?;
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_owned());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3002".to_owned());
    let jwt_secret = std::env::var("JWT_SECRET").context("JWT_SECRET must be set")?;

    // Connect to PostgreSQL
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("failed to connect to database")?;

    tracing::info!("connected to database");

    // Create rate limiter
    let rate_limiter = Arc::new(RateLimiter::new());

    // Build router with auth routes
    let app = auth_routes(pool, rate_limiter, jwt_secret);

    // Start server
    let bind_addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .context(format!("failed to bind to {bind_addr}"))?;

    tracing::info!("auth service listening on {bind_addr}");
    axum::serve(listener, app).await.context("server error")?;

    Ok(())
}
