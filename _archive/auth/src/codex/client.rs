//! HTTP client for OAuth token operations.
//!
//! Handles token exchange, refresh, and API requests to the Codex backend.

use std::time::Duration;

use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use super::constants::{CLIENT_ID, CODEX_API_BASE, CODEX_RESPONSES_PATH, ORIGINATOR, TOKEN_URL};
use super::token::AuthState;
use crate::error::{AuthError, AuthResult};
use crate::pkce::PkceCodes;

/// Tokens returned from OAuth token exchange.
#[derive(Debug, Clone)]
pub struct ExchangedTokens {
	/// The ID token containing user claims.
	pub id_token: String,
	/// The access token for API requests.
	pub access_token: String,
	/// The refresh token for obtaining new access tokens.
	pub refresh_token: String,
}

#[derive(Deserialize)]
struct TokenResponse {
	id_token: String,
	access_token: String,
	refresh_token: String,
}

#[derive(Deserialize)]
struct RefreshResponse {
	id_token: Option<String>,
	access_token: Option<String>,
	refresh_token: Option<String>,
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code_for_tokens(
	issuer: &str,
	client_id: &str,
	redirect_uri: &str,
	pkce: &PkceCodes,
	code: &str,
) -> AuthResult<ExchangedTokens> {
	let client = Client::new();

	let body = format!(
		"grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
		urlencoding::encode(code),
		urlencoding::encode(redirect_uri),
		urlencoding::encode(client_id),
		urlencoding::encode(&pkce.verifier)
	);

	let response = client
		.post(format!("{issuer}/oauth/token"))
		.header(CONTENT_TYPE, "application/x-www-form-urlencoded")
		.body(body)
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

	Ok(ExchangedTokens {
		id_token: tokens.id_token,
		access_token: tokens.access_token,
		refresh_token: tokens.refresh_token,
	})
}

/// Refresh an access token using a refresh token.
pub async fn refresh_access_token(refresh_token: &str) -> AuthResult<ExchangedTokens> {
	let client = Client::new();

	#[derive(Serialize)]
	struct RefreshRequest<'a> {
		client_id: &'static str,
		grant_type: &'static str,
		refresh_token: &'a str,
		scope: &'static str,
	}

	let request = RefreshRequest {
		client_id: CLIENT_ID,
		grant_type: "refresh_token",
		refresh_token,
		scope: "openid profile email",
	};

	let response = client
		.post(TOKEN_URL)
		.header(CONTENT_TYPE, "application/json")
		.json(&request)
		.timeout(Duration::from_secs(60))
		.send()
		.await
		.map_err(|e| AuthError::Network(e.to_string()))?;

	let status = response.status();
	if status == reqwest::StatusCode::UNAUTHORIZED {
		let text = response.text().await.unwrap_or_default();
		return Err(classify_refresh_error(&text));
	}

	if !status.is_success() {
		let text = response.text().await.unwrap_or_default();
		return Err(AuthError::TokenRefresh(format!("status {status}: {text}")));
	}

	let tokens: RefreshResponse = response
		.json()
		.await
		.map_err(|e| AuthError::TokenRefresh(format!("invalid response: {e}")))?;

	Ok(ExchangedTokens {
		id_token: tokens.id_token.unwrap_or_default(),
		access_token: tokens
			.access_token
			.ok_or_else(|| AuthError::TokenRefresh("missing access_token".into()))?,
		refresh_token: tokens
			.refresh_token
			.ok_or_else(|| AuthError::TokenRefresh("missing refresh_token".into()))?,
	})
}

fn classify_refresh_error(body: &str) -> AuthError {
	let code = serde_json::from_str::<serde_json::Value>(body)
		.ok()
		.and_then(|v| {
			v.get("error")
				.and_then(|e| e.as_str().or_else(|| e.get("code")?.as_str()))
				.or_else(|| v.get("code")?.as_str())
				.map(str::to_owned)
		});

	let message = match code.as_deref() {
		Some("refresh_token_expired") => "refresh token expired",
		Some("refresh_token_reused") => "refresh token already used",
		Some("refresh_token_invalidated") => "refresh token revoked",
		_ => "token refresh failed",
	};

	AuthError::TokenRefresh(message.into())
}

/// Authenticated client for making Codex API requests.
#[derive(Clone)]
pub struct CodexClient {
	client: Client,
	auth: AuthState,
}

impl CodexClient {
	/// Create a new client with the given auth state.
	pub fn new(auth: AuthState) -> AuthResult<Self> {
		if !auth.is_authenticated() {
			return Err(AuthError::NotAuthenticated);
		}

		let client = Client::builder()
			.timeout(Duration::from_secs(120))
			.build()
			.map_err(|e| AuthError::Network(e.to_string()))?;

		Ok(Self { client, auth })
	}

	/// Get the bearer token.
	pub fn bearer_token(&self) -> Option<&str> {
		self.auth.bearer_token()
	}

	/// Get the account ID.
	pub fn account_id(&self) -> Option<&str> {
		self.auth.account_id()
	}

	/// Make an authenticated POST request to the Codex responses endpoint.
	pub async fn post_responses(&self, body: serde_json::Value) -> AuthResult<reqwest::Response> {
		let url = format!("{CODEX_API_BASE}{CODEX_RESPONSES_PATH}");

		let Some(token) = self.bearer_token() else {
			return Err(AuthError::NotAuthenticated);
		};

		let mut request = self
			.client
			.post(&url)
			.header(AUTHORIZATION, format!("Bearer {token}"))
			.header(CONTENT_TYPE, "application/json")
			.header("OpenAI-Beta", "responses=experimental")
			.header("originator", ORIGINATOR);

		if let Some(account_id) = self.account_id() {
			request = request.header("ChatGPT-Account-ID", account_id);
		}

		let response = request
			.json(&body)
			.send()
			.await
			.map_err(|e| AuthError::Network(e.to_string()))?;

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn client_requires_auth() {
		let empty_auth = AuthState::default();
		let result = CodexClient::new(empty_auth);
		assert!(matches!(result, Err(AuthError::NotAuthenticated)));
	}

	#[test]
	fn client_accepts_api_key() {
		let auth = AuthState::from_api_key("sk-test".into());
		let client = CodexClient::new(auth).unwrap();
		assert_eq!(client.bearer_token(), Some("sk-test"));
		assert!(client.account_id().is_none());
	}
}
