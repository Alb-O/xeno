//! OpenAI Codex OAuth authentication.
//!
//! Provides OAuth 2.0 + PKCE authentication against OpenAI's auth server,
//! enabling access to the Codex API through ChatGPT subscriptions.

mod client;
mod constants;
mod server;
mod storage;
mod token;

pub use client::{CodexClient, refresh_access_token};
pub use server::{LoginConfig, start_login};
pub use storage::{load_auth, logout};
pub use token::{AuthState, TokenData};
