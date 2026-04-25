//! 钉钉 DingTalk 平台适配器
//!
//! 本模块实现钉钉平台的 Webhook 适配器，支持 Stream Mode 连接。

mod client;
mod dingtalk;
mod error;

pub use client::DingTalkStreamClient;
pub use dingtalk::DingTalkAdapter;
pub use error::DingTalkError;
