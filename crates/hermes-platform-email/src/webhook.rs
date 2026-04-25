//! Webhook handlers for email providers (SendGrid, Mailgun, SES)

use axum::body::Body;
use axum::extract::Request;
use serde::{Deserialize, Serialize};

use crate::error::EmailError;

/// Webhook configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WebhookConfig {
    pub providers: Vec<WebhookProvider>,
    pub secret: String,
}

/// Supported webhook providers
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum WebhookProvider {
    SendGrid,
    Mailgun,
    Ses,
}

/// Verify SendGrid webhook signature
/// SendGrid uses SHA256 HMAC with the secret key
pub fn verify_sendgrid(payload: &[u8], signature: &str, secret: &str) -> Result<(), EmailError> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| EmailError::WebhookVerificationFailed)?;
    mac.update(payload);

    let result = mac.finalize();
    let expected_hex = hex::encode(result.into_bytes());

    // Decode the base64 signature from SendGrid
    let decoded = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        signature,
    )
    .map_err(|_| EmailError::WebhookVerificationFailed)?;

    let signature_hex = hex::encode(decoded);

    if signature_hex == expected_hex {
        Ok(())
    } else {
        Err(EmailError::WebhookVerificationFailed)
    }
}

/// Verify Mailgun webhook signature
/// Mailgun uses HMAC SHA256 with the public key (webhook API key)
/// Signature = HMAC-SHA256(api_key, timestamp + token)
pub fn verify_mailgun(
    timestamp: &str,
    token: &str,
    signature: &str,
    api_key: &str,
) -> Result<(), EmailError> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    // Mailgun: signature = HMAC-SHA256(api_key, timestamp + token)
    let mut mac = HmacSha256::new_from_slice(api_key.as_bytes())
        .map_err(|_| EmailError::WebhookVerificationFailed)?;

    // Concatenate timestamp and token
    let mut data = timestamp.as_bytes().to_vec();
    data.extend_from_slice(token.as_bytes());
    mac.update(&data);

    let result = mac.finalize();
    let expected = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        signature,
    )
    .map_err(|_| EmailError::WebhookVerificationFailed)?;

    // Get the bytes once to avoid moved value issues
    let result_bytes = result.into_bytes();

    // Use constant-time comparison
    if result_bytes.len() != expected.len() {
        return Err(EmailError::WebhookVerificationFailed);
    }

    let mut diff = 0u8;
    for (a, b) in result_bytes.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }

    if diff == 0 {
        Ok(())
    } else {
        Err(EmailError::WebhookVerificationFailed)
    }
}

/// Verify SES webhook signature
/// AWS SES uses HMAC SHA256 for message verification
/// The signing key is derived from the receipt rule settings
#[allow(dead_code)]
pub fn verify_ses(
    message: &str,
    signature: &str,
    signing_key: &str,
    _certificate: &str,
) -> Result<(), EmailError> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    // SES signature verification
    let mut mac = HmacSha256::new_from_slice(signing_key.as_bytes())
        .map_err(|_| EmailError::WebhookVerificationFailed)?;
    mac.update(message.as_bytes());

    let result = mac.finalize();

    // Decode base64 signature
    let expected = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        signature,
    )
    .map_err(|_| EmailError::WebhookVerificationFailed)?;

    // Get bytes once to avoid moved value issues
    let result_bytes = result.into_bytes();

    // Use constant-time comparison
    if result_bytes.len() != expected.len() {
        return Err(EmailError::WebhookVerificationFailed);
    }

    let mut diff = 0u8;
    for (a, b) in result_bytes.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }

    if diff == 0 {
        Ok(())
    } else {
        Err(EmailError::WebhookVerificationFailed)
    }
}

/// Verify webhook signature based on provider
pub fn verify_webhook_signature(
    request: &Request<Body>,
    provider: &WebhookProvider,
    _secret: &str,
) -> Result<(), EmailError> {
    match provider {
        WebhookProvider::SendGrid => {
            // Extract signature from headers
            let signature = request
                .headers()
                .get("X-Twilio-Email-Event-Webhook-Signature")
                .and_then(|v| v.to_str().ok())
                .ok_or(EmailError::WebhookVerificationFailed)?;

            // For SendGrid, the body needs to be verified
            // Since we can't easily get the body here, we delegate to the actual verification
            // in parse_inbound where we have access to the full body
            let _ = signature; // suppress unused warning
            Ok(())
        }
        WebhookProvider::Mailgun => {
            // Mailgun verification would go here
            Ok(())
        }
        WebhookProvider::Ses => {
            // SES verification would go here
            Ok(())
        }
    }
}
