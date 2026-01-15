//! Authentication providers for Xeno.
//!
//! This crate provides authentication support for various AI services:
//!
//! - **Codex**: OpenAI Codex OAuth via ChatGPT subscriptions
//! - **Claude**: Anthropic Claude OAuth via Claude Pro/Max subscriptions

#![warn(missing_docs)]

pub mod claude;
pub mod codex;
mod error;
mod pkce;
mod xdg;

// Re-export shared types at crate root
pub use error::{AuthError, AuthResult};
pub use pkce::{PkceCodes, generate_state};
pub use xdg::{default_config_dir, default_data_dir};
