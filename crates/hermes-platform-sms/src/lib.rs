//! SMS Platform Adapter
//!
//! 实现 Twilio SMS Webhook 集成
//!
//! 支持：
//! - 接收入站 SMS
//! - 发送出站 SMS
//! - Webhook 签名验证
//! - 长消息自动分片

pub mod client;
pub mod error;
pub mod sms;

pub use client::{TwilioClient, TwilioMessageResponse, TwilioWebhookPayload};
pub use error::SmsError;
pub use sms::SmsAdapter;
