//! Honcho Provider - Pluggable user modeling via Honcho SDK
//!
//! This module provides a MemoryProvider implementation that integrates
//! with the Honcho SDK for cross-session user modeling.

pub mod client;
pub mod session;

pub use client::HonchoClient;
pub use session::HonchoSessionManager;