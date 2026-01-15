//! Anthropic Claude OAuth authentication.
//!
//! Provides OAuth 2.0 + PKCE authentication against Anthropic's auth server,
//! supporting both Claude Pro/Max subscriptions and API key creation.
//!
//! # Login Modes
//!
//! - [`LoginMode::Max`]: OAuth for Claude Pro/Max subscription users.
//! - [`LoginMode::Console`]: OAuth to create a persistent API key.

mod client;
mod constants;
mod login;
mod storage;
mod token;

pub use login::{LoginMode, LoginSession, complete_login, start_login};
pub use storage::{load_auth, logout};
pub use token::{AuthState, OAuthTokens};
