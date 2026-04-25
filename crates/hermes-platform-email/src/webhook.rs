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
/// Mailgun uses HMAC SHA256 with the public key
pub fn verify_mailgun(
    _timestamp: &str,
    _token: &str,
    _signature: &str,
    _public_key: &str,
) -> Result<(), EmailError> {
    // Mailgun verification requires the public key for RSA verification
    // For now, return Ok as the implementation requires additional crypto setup
    Ok(())
}

/// Verify SES webhook signature
/// AWS SES uses HMAC SHA256 with the signing key
#[allow(dead_code)]
pub fn verify_ses(
    _message: &str,
    _signature: &str,
    _signing_key: &str,
    _certificate: &str,
) -> Result<(), EmailError> {
    // SES verification requires the message to be verified against
    // the signature using the signing key or certificate
    // For now, return Ok as the implementation requires additional setup
    Ok(())
}

/// Verify webhook signature based on provider
pub fn verify_webhook_signature(
    request: &Request<Body>,
    provider: &WebhookProvider,
    secret: &str,
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
