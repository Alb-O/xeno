//! Claude OAuth login flow.
//!
//! Unlike Codex, Claude uses a "code paste" method where the user
//! copies the authorization code from the browser.

use std::path::PathBuf;

use super::client::{create_api_key, exchange_code_for_tokens};
use super::constants::{AUTHORIZE_URL_CONSOLE, AUTHORIZE_URL_MAX, CLIENT_ID, REDIRECT_URI, SCOPE};
use super::storage::save_auth;
use super::token::AuthState;
use crate::error::AuthResult;
use crate::pkce::PkceCodes;

/// Login mode for Claude OAuth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginMode {
	/// Claude Pro/Max subscription (OAuth tokens).
	Max,
	/// Console login to create an API key.
	Console,
}

/// Pending login session.
#[derive(Debug)]
pub struct LoginSession {
	/// Authorization URL to open in browser.
	pub auth_url: String,
	/// PKCE verifier (needed for code exchange).
	pub verifier: String,
	/// Login mode.
	pub mode: LoginMode,
	/// Data directory for storing auth state.
	pub data_dir: PathBuf,
}

/// Start Claude OAuth login flow.
///
/// Returns a login session with the authorization URL. The user must:
/// 1. Open the URL in their browser
/// 2. Complete authentication
/// 3. Copy the authorization code
/// 4. Call [`complete_login`] with the code
pub fn start_login(data_dir: PathBuf, mode: LoginMode) -> LoginSession {
	let pkce = PkceCodes::generate();

	let base_url = match mode {
		LoginMode::Max => AUTHORIZE_URL_MAX,
		LoginMode::Console => AUTHORIZE_URL_CONSOLE,
	};

	let auth_url = build_authorize_url(base_url, &pkce);

	LoginSession {
		auth_url,
		verifier: pkce.verifier,
		mode,
		data_dir,
	}
}

/// Complete the login flow with the authorization code.
///
/// The code should be copied from the browser after authentication.
/// It may be in format `code#state` or just `code`.
pub async fn complete_login(session: &LoginSession, code: &str) -> AuthResult<()> {
	let tokens = exchange_code_for_tokens(code, &session.verifier).await?;

	let state = match session.mode {
		LoginMode::Max => {
			AuthState::from_oauth(tokens.access_token, tokens.refresh_token, tokens.expires_at)
		}
		LoginMode::Console => {
			let api_key = create_api_key(&tokens.access_token).await?;
			AuthState::from_api_key(api_key)
		}
	};

	save_auth(&session.data_dir, &state)?;
	Ok(())
}

fn build_authorize_url(base_url: &str, pkce: &PkceCodes) -> String {
	let params = [
		("code", "true"),
		("client_id", CLIENT_ID),
		("response_type", "code"),
		("redirect_uri", REDIRECT_URI),
		("scope", SCOPE),
		("code_challenge", &pkce.challenge),
		("code_challenge_method", "S256"),
		("state", &pkce.verifier),
	];

	let query = params
		.into_iter()
		.map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
		.collect::<Vec<_>>()
		.join("&");

	format!("{base_url}?{query}")
}
