//! OpenAI Codex OAuth authentication for Xeno.
//!
//! This crate provides OAuth 2.0 + PKCE authentication against OpenAI's
//! auth server, enabling access to the Codex API through ChatGPT subscriptions.
//!
//! # Authentication Flow
//!
//! 1. Generate PKCE codes (verifier + challenge)
//! 2. Start local HTTP server on port 1455
//! 3. Open browser to OpenAI authorization URL
//! 4. User authenticates in browser
//! 5. Browser redirects to local server with auth code
//! 6. Exchange auth code for tokens
//! 7. Store tokens to ~/.xeno/auth.json
//!
//! # Example
//!
//! ```ignore
//! use xeno_auth_codex::{LoginConfig, start_login, default_data_dir};
//!
//! // Start OAuth login flow
//! let config = LoginConfig::new(default_data_dir()?);
//! let server = start_login(config)?;
//!
//! println!("Open: {}", server.auth_url);
//!
//! // Wait for completion
//! server.wait().await?;
//! ```
//!
//! # API Key Authentication
//!
//! For direct API key usage:
//!
//! ```ignore
//! use xeno_auth_codex::{default_data_dir, login_with_api_key};
//!
//! let data_dir = default_data_dir()?;
//! login_with_api_key(&data_dir, "sk-...")?;
//! ```

#![warn(missing_docs)]

mod client;
mod constants;
mod error;
mod pkce;
mod server;
mod storage;
mod token;

// Re-export primary types
pub use client::CodexClient;
pub use client::ExchangedTokens;
pub use client::exchange_code_for_tokens;
pub use client::refresh_access_token;
pub use constants::CLIENT_ID;
pub use constants::CODEX_API_BASE;
pub use constants::ISSUER;
pub use constants::ORIGINATOR;
pub use error::AuthError;
pub use error::AuthResult;
pub use pkce::PkceCodes;
pub use pkce::generate_state;
pub use server::LoginConfig;
pub use server::LoginServer;
pub use server::ShutdownHandle;
pub use server::start_login;
pub use storage::auth_file_path;
pub use storage::default_config_dir;
pub use storage::default_data_dir;
pub use storage::delete_auth;
pub use storage::load_auth;
pub use storage::login_with_api_key;
pub use storage::logout;
pub use storage::save_auth;
pub use token::AuthState;
pub use token::IdTokenClaims;
pub use token::TokenData;
pub use token::jwt_auth_claims;
pub use token::parse_id_token;

/// Authentication mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Using an OpenAI API key.
    ApiKey,
    /// Using ChatGPT OAuth tokens.
    ChatGPT,
}

impl AuthState {
    /// Get the current authentication mode.
    pub fn mode(&self) -> Option<AuthMode> {
        if self.api_key.is_some() {
            Some(AuthMode::ApiKey)
        } else if self.tokens.is_some() {
            Some(AuthMode::ChatGPT)
        } else {
            None
        }
    }
}
