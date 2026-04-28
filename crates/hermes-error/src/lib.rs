//! ## hermes-error
//!
//! 共享存储错误类型定义。
//!
//! 本模块定义了 `StorageError` 枚举，供 `hermes-memory` 等模块使用，
//! 避免通过 `hermes-core` 产生循环依赖。

use thiserror::Error;

/// 共享存储错误类型
///
/// 定义了存储层可能发生的各类错误，包括连接、查询、迁移等操作。
#[derive(Error, Debug)]
pub enum StorageError {
    /// 连接失败错误
    #[error("Connection failed: {0}")]
    Connection(String),

    /// 查询失败错误
    #[error("Query failed: {0}")]
    Query(String),

    /// 迁移失败错误
    #[error("Migration failed: {0}")]
    Migration(String),

    /// 数据库繁忙错误，需要重试
    #[error("Busy, try again")]
    Busy,

    /// 超过最大重试次数错误
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
}
