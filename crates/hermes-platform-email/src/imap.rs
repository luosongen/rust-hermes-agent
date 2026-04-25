//! IMAP poller for receiving emails

/// IMAP configuration
#[derive(Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub poll_interval_secs: u64,
}

/// IMAP poller
pub struct ImapPoller {
    config: ImapConfig,
}

impl ImapPoller {
    pub fn new(config: ImapConfig) -> Self {
        Self { config }
    }
}
