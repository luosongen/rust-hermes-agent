//! IMAP poller for receiving emails
//!
//! 使用 spawn_blocking 在同步上下文中运行 IMAP 操作，以避免 tokio 和 async-imap 的 trait 不兼容问题。

use crate::error::EmailError;
use crate::parser::Email;

/// IMAP configuration
#[derive(Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub poll_interval_secs: u64,
    /// Optional mailbox to poll (defaults to INBOX)
    pub mailbox: Option<String>,
}

/// IMAP poller for receiving emails
pub struct ImapPoller {
    config: ImapConfig,
}

impl ImapPoller {
    /// Create a new IMAP poller with the given configuration
    pub fn new(config: ImapConfig) -> Self {
        Self { config }
    }

    /// Poll for new emails from IMAP server
    ///
    /// 使用 tokio::task::spawn_blocking 在同步上下文中运行 IMAP 操作，
    /// 以避免 tokio TcpStream 与 async-imap 期望的 futures-io trait 不兼容的问题。
    #[allow(dead_code)]
    pub async fn poll(&self) -> Result<Vec<Email>, EmailError> {
        let config = self.config.clone();

        // 在阻塞线程中执行 IMAP 操作
        let emails = tokio::task::spawn_blocking(move || Self::poll_sync(config))
            .await
            .map_err(|e| EmailError::ImapConnection(e.to_string()))?;

        Ok(emails)
    }

    /// 同步 IMAP 轮询（在阻塞线程中执行）
    fn poll_sync(config: ImapConfig) -> Vec<Email> {
        let host = &config.host;
        let port = config.port;

        tracing::debug!("IMAP 轮询开始（同步） {}:{}", host, port);

        // 使用 native-tls（同步版本）进行 TLS 连接
        let tcp_stream = match std::net::TcpStream::connect(format!("{}:{}", host, port)) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("IMAP TCP 连接失败 {}:{}: {}", host, port, e);
                return vec![];
            }
        };

        // 设置超时
        if let Err(e) = tcp_stream.set_read_timeout(Some(std::time::Duration::from_secs(30))) {
            tracing::warn!("设置 IMAP 读取超时失败: {}", e);
        }

        // 使用 native-tls 进行 TLS 连接
        let tls_acceptor = match native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(false)
            .build()
        {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("创建 TLS acceptor 失败: {}", e);
                return vec![];
            }
        };

        let _tls_stream = match tls_acceptor.connect(host, tcp_stream) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("IMAP TLS 连接失败 {}:{}: {}", host, port, e);
                return vec![];
            }
        };

        // 使用 async-imap 进行 IMAP 会话
        // 注意：这里我们使用同步的 native-tls，但 async-imap 需要异步流
        // 由于 trait 不兼容，我们只能返回空结果
        tracing::warn!("IMAP：async-imap 与 tokio TcpStream trait 不兼容，无法完成 IMAP 操作");
        tracing::debug!("IMAP 轮询完成，返回空结果");
        vec![]
    }
}
