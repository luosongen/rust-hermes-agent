//! Delegate — subagent delegation support.
//!
//! ## Module structure
//! - **types** — `DelegateParams`, `DelegateResult`, `DelegateTask`, `DelegateStatus`,
//!   `BatchDelegateResult`, tool constants (`BLOCKED_TOOLS`, `MAX_DELEGATION_DEPTH`, etc.)
//! - **delegate_tool** — lives in `hermes-tool-registry` (implements the `Tool` trait)

pub mod types;
pub use types::*;
