//! School Run authentication service.
//!
//! Provides OTP-based authentication with JWT token issuance
//! for the School Run dashboard. Only pre-approved email addresses
//! can authenticate.

pub mod db;
pub mod email;
pub mod error;
pub mod jwt;
pub mod otp;
pub mod rate_limit;
pub mod routes;
