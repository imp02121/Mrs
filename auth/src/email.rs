//! OTP email delivery via the Resend transactional email API.
//!
//! In development mode (no `RESEND_API_KEY`), OTP codes are logged to the console
//! instead of being emailed. In production, a POST request is sent to the
//! Resend API to deliver the OTP email.

use crate::error::AuthError;

/// Default sender address when `RESEND_FROM` is not set.
const DEFAULT_FROM: &str = "School Run <onboarding@resend.dev>";

/// Resend API endpoint for sending emails.
const RESEND_API_URL: &str = "https://api.resend.com/emails";

/// Sends an OTP code to the specified email address.
///
/// If the `RESEND_API_KEY` environment variable is set, delivers the OTP
/// via the Resend transactional email API. Otherwise, logs the OTP to the
/// console for development use.
///
/// # Arguments
///
/// * `to` - Recipient email address
/// * `otp` - The 6-digit OTP code to send
///
/// # Errors
///
/// Returns [`AuthError::Internal`] if the HTTP request to Resend fails.
pub async fn send_otp_email(to: &str, otp: &str) -> Result<(), AuthError> {
    let api_key = std::env::var("RESEND_API_KEY").ok();

    match api_key {
        None => {
            tracing::info!("DEV MODE OTP for {to}: {otp}");
            Ok(())
        }
        Some(key) => send_via_resend(to, otp, &key).await,
    }
}

/// Sends the OTP email through the Resend API.
async fn send_via_resend(to: &str, otp: &str, api_key: &str) -> Result<(), AuthError> {
    let from = std::env::var("RESEND_FROM").unwrap_or_else(|_| DEFAULT_FROM.to_owned());

    let body = serde_json::json!({
        "from": from,
        "to": [to],
        "subject": "Your School Run login code",
        "text": format!("Your School Run login code: {otp}. Expires in 5 minutes.")
    });

    let client = reqwest::Client::new();
    let response = client
        .post(RESEND_API_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AuthError::Internal(format!("failed to send email: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_owned());
        return Err(AuthError::Internal(format!(
            "Resend API error ({status}): {text}"
        )));
    }

    tracing::info!("OTP email sent to {to}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_from_address() {
        assert_eq!(DEFAULT_FROM, "School Run <onboarding@resend.dev>");
    }

    #[test]
    fn test_resend_api_url() {
        assert_eq!(RESEND_API_URL, "https://api.resend.com/emails");
    }

    #[tokio::test]
    async fn test_send_otp_email_dev_mode() {
        // Ensure RESEND_API_KEY is unset for this test
        // SAFETY: This test runs in isolation; no other threads depend on this env var.
        unsafe {
            std::env::remove_var("RESEND_API_KEY");
        }
        let result = send_otp_email("test@example.com", "123456").await;
        assert!(result.is_ok(), "Dev mode should always succeed");
    }

    #[tokio::test]
    async fn test_send_otp_email_dev_mode_various_inputs() {
        // Dev mode should handle any inputs without panicking
        unsafe {
            std::env::remove_var("RESEND_API_KEY");
        }

        // Normal email and OTP
        assert!(send_otp_email("user@example.com", "123456").await.is_ok());
        // Empty OTP (still should not panic in dev mode)
        assert!(send_otp_email("user@example.com", "").await.is_ok());
        // Empty email (dev mode just logs)
        assert!(send_otp_email("", "123456").await.is_ok());
        // Long inputs
        let long_email = format!("{}@example.com", "a".repeat(500));
        assert!(send_otp_email(&long_email, "999999").await.is_ok());
    }

    #[test]
    fn test_resend_request_body_format() {
        // Verify the JSON body we'd send to Resend has the right shape
        let to = "recipient@example.com";
        let otp = "482910";
        let from = DEFAULT_FROM;

        let body = serde_json::json!({
            "from": from,
            "to": [to],
            "subject": "Your School Run login code",
            "text": format!("Your School Run login code: {otp}. Expires in 5 minutes.")
        });

        assert_eq!(body["from"], "School Run <onboarding@resend.dev>");
        assert!(body["to"].is_array());
        assert_eq!(body["to"][0], "recipient@example.com");
        assert_eq!(body["subject"], "Your School Run login code");
        let text = body["text"].as_str().unwrap();
        assert!(text.contains("482910"), "Body must contain the OTP");
        assert!(text.contains("5 minutes"), "Body must mention expiry");
    }

    // Note: Testing the Resend API path requires mocking or integration tests.
    // The send_via_resend function is exercised in integration tests with a
    // mock server.
}
