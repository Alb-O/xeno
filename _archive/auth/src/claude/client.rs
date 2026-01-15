//! HTTP client for Claude OAuth token operations.

use std::time::Duration;

use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use super::constants::{CLIENT_ID, CREATE_API_KEY_URL, REDIRECT_URI, TOKEN_URL};
use crate::error::{AuthError, AuthResult};

/// Tokens returned from OAuth token exchange.
#[derive(Debug, Clone)]
pub struct ExchangedTokens {
	/// Access token for API requests.
	pub access_token: String,
	/// Refresh token for obtaining new tokens.
	pub refresh_token: String,
	/// Expiration time in milliseconds since epoch.
	pub expires_at: i64,
}

#[derive(Deserialize)]
struct TokenResponse {
	access_token: String,
	refresh_token: String,
	expires_in: i64,
}

/// Exchange authorization code for tokens.
///
/// The code should be in format `code#state` as returned by Anthropic's OAuth.
pub async fn exchange_code_for_tokens(code: &str, verifier: &str) -> AuthResult<ExchangedTokens> {
	let (auth_code, state) = parse_code_state(code);

	#[derive(Serialize)]
	struct TokenRequest<'a> {
		code: &'a str,
		state: Option<&'a str>,
		grant_type: &'static str,
		client_id: &'static str,
		redirect_uri: &'static str,
		code_verifier: &'a str,
	}

	let request = TokenRequest {
		code: auth_code,
		state,
		grant_type: "authorization_code",
		client_id: CLIENT_ID,
		redirect_uri: REDIRECT_URI,
		code_verifier: verifier,
	};

	let client = Client::new();
	let response = client
		.post(TOKEN_URL)
		.header(CONTENT_TYPE, "application/json")
		.json(&request)
		.timeout(Duration::from_secs(30))
		.send()
		.await
		.map_err(|e| AuthError::Network(e.to_string()))?;

	if !response.status().is_success() {
		let status = response.status();
		let text = response.text().await.unwrap_or_default();
		return Err(AuthError::TokenExchange(format!("status {status}: {text}")));
	}

	let tokens: TokenResponse = response
		.json()
		.await
		.map_err(|e| AuthError::TokenExchange(format!("invalid response: {e}")))?;

	let now_ms = chrono::Utc::now().timestamp_millis();
	Ok(ExchangedTokens {
		access_token: tokens.access_token,
		refresh_token: tokens.refresh_token,
		expires_at: now_ms + tokens.expires_in * 1000,
	})
}

/// Refresh access token using refresh token.
#[allow(dead_code, reason = "needed when OAuth tokens expire")]
pub(crate) async fn refresh_access_token(refresh_token: &str) -> AuthResult<ExchangedTokens> {
	#[derive(Serialize)]
	struct RefreshRequest<'a> {
		grant_type: &'static str,
		refresh_token: &'a str,
		client_id: &'static str,
	}

	let request = RefreshRequest {
		grant_type: "refresh_token",
		refresh_token,
		client_id: CLIENT_ID,
	};

	let client = Client::new();
	let response = client
		.post(TOKEN_URL)
		.header(CONTENT_TYPE, "application/json")
		.json(&request)
		.timeout(Duration::from_secs(30))
		.send()
		.await
		.map_err(|e| AuthError::Network(e.to_string()))?;

	if !response.status().is_success() {
		let status = response.status();
		let text = response.text().await.unwrap_or_default();
		return Err(AuthError::TokenRefresh(format!("status {status}: {text}")));
	}

	let tokens: TokenResponse = response
		.json()
		.await
		.map_err(|e| AuthError::TokenRefresh(format!("invalid response: {e}")))?;

	let now_ms = chrono::Utc::now().timestamp_millis();
	Ok(ExchangedTokens {
		access_token: tokens.access_token,
		refresh_token: tokens.refresh_token,
		expires_at: now_ms + tokens.expires_in * 1000,
	})
}

/// Create a persistent API key using OAuth credentials.
pub async fn create_api_key(access_token: &str) -> AuthResult<String> {
	#[derive(Deserialize)]
	struct ApiKeyResponse {
		raw_key: String,
	}

	let client = Client::new();
	let response = client
		.post(CREATE_API_KEY_URL)
		.header(CONTENT_TYPE, "application/json")
		.header(AUTHORIZATION, format!("Bearer {access_token}"))
		.timeout(Duration::from_secs(30))
		.send()
		.await
		.map_err(|e| AuthError::Network(e.to_string()))?;

	if !response.status().is_success() {
		let status = response.status();
		let text = response.text().await.unwrap_or_default();
		return Err(AuthError::TokenExchange(format!(
			"API key creation failed: status {status}: {text}"
		)));
	}

	let result: ApiKeyResponse = response
		.json()
		.await
		.map_err(|e| AuthError::TokenExchange(format!("invalid response: {e}")))?;

	Ok(result.raw_key)
}

/// Parse code#state format from Anthropic callback.
fn parse_code_state(input: &str) -> (&str, Option<&str>) {
	match input.split_once('#') {
		Some((code, state)) => (code, Some(state)),
		None => (input, None),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_code_with_state() {
		let (code, state) = parse_code_state("abc123#xyz789");
		assert_eq!(code, "abc123");
		assert_eq!(state, Some("xyz789"));
	}

	#[test]
	fn parse_code_without_state() {
		let (code, state) = parse_code_state("abc123");
		assert_eq!(code, "abc123");
		assert_eq!(state, None);
	}
}
