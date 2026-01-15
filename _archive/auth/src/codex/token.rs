//! Token types and JWT parsing utilities for Codex.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::constants::JWT_AUTH_CLAIM;
use crate::error::{AuthError, AuthResult};

/// Parsed ID token claims from OpenAI OAuth.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdTokenClaims {
	/// User's email address.
	#[serde(default)]
	pub email: Option<String>,

	/// Whether the email is verified.
	#[serde(default)]
	pub email_verified: Option<bool>,

	/// ChatGPT subscription plan type (free, plus, pro, team, etc).
	#[serde(default)]
	pub plan_type: Option<String>,

	/// ChatGPT account/workspace ID.
	#[serde(default)]
	pub account_id: Option<String>,

	/// ChatGPT user ID.
	#[serde(default)]
	pub user_id: Option<String>,

	/// Raw JWT string for re-encoding.
	#[serde(skip)]
	pub raw_jwt: String,
}

/// Complete token set from OAuth flow.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenData {
	/// Parsed ID token claims.
	pub id_token: IdTokenClaims,

	/// Access token for API requests.
	pub access_token: String,

	/// Refresh token for obtaining new access tokens.
	pub refresh_token: String,

	/// Account ID extracted from JWT (for API headers).
	#[serde(default)]
	pub account_id: Option<String>,
}

/// Stored authentication state for Codex.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthState {
	/// Optional API key (for API key auth mode).
	#[serde(rename = "OPENAI_API_KEY")]
	pub api_key: Option<String>,

	/// OAuth tokens (for ChatGPT auth mode).
	pub tokens: Option<TokenData>,

	/// Last token refresh timestamp.
	pub last_refresh: Option<DateTime<Utc>>,
}

impl AuthState {
	/// Create an auth state with just an API key.
	pub fn from_api_key(api_key: String) -> Self {
		Self {
			api_key: Some(api_key),
			tokens: None,
			last_refresh: None,
		}
	}

	/// Create an auth state with OAuth tokens.
	pub fn from_tokens(tokens: TokenData) -> Self {
		Self {
			api_key: None,
			tokens: Some(tokens),
			last_refresh: Some(Utc::now()),
		}
	}

	/// Check if we have valid authentication.
	pub fn is_authenticated(&self) -> bool {
		self.api_key.is_some() || self.tokens.is_some()
	}

	/// Get the bearer token for API requests.
	pub fn bearer_token(&self) -> Option<&str> {
		self.api_key
			.as_deref()
			.or_else(|| self.tokens.as_ref().map(|t| t.access_token.as_str()))
	}

	/// Get the account ID for API headers.
	pub fn account_id(&self) -> Option<&str> {
		self.tokens.as_ref().and_then(|t| t.account_id.as_deref())
	}
}

/// Parse an ID token JWT and extract claims.
pub fn parse_id_token(jwt: &str) -> AuthResult<IdTokenClaims> {
	let parts: Vec<&str> = jwt.split('.').collect();
	if parts.len() != 3 {
		return Err(AuthError::InvalidToken("JWT must have 3 parts".into()));
	}

	let payload_bytes = URL_SAFE_NO_PAD
		.decode(parts[1])
		.map_err(|e| AuthError::InvalidToken(format!("base64 decode failed: {e}")))?;

	let payload: serde_json::Value = serde_json::from_slice(&payload_bytes)
		.map_err(|e| AuthError::InvalidToken(format!("JSON parse failed: {e}")))?;

	let auth_claims = payload
		.get(JWT_AUTH_CLAIM)
		.and_then(|v| v.as_object())
		.cloned()
		.unwrap_or_default();

	Ok(IdTokenClaims {
		email: payload
			.get("email")
			.and_then(|v| v.as_str())
			.map(String::from),
		email_verified: payload.get("email_verified").and_then(|v| v.as_bool()),
		plan_type: auth_claims
			.get("chatgpt_plan_type")
			.and_then(|v| v.as_str())
			.map(String::from),
		account_id: auth_claims
			.get("chatgpt_account_id")
			.and_then(|v| v.as_str())
			.map(String::from),
		user_id: auth_claims
			.get("chatgpt_user_id")
			.and_then(|v| v.as_str())
			.map(String::from),
		raw_jwt: jwt.to_string(),
	})
}

/// Extract auth claims from a JWT without full validation.
pub fn jwt_auth_claims(jwt: &str) -> serde_json::Map<String, serde_json::Value> {
	let parts: Vec<&str> = jwt.split('.').collect();
	if parts.len() != 3 || parts[1].is_empty() {
		return serde_json::Map::new();
	}

	let Ok(bytes) = URL_SAFE_NO_PAD.decode(parts[1]) else {
		return serde_json::Map::new();
	};

	let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
		return serde_json::Map::new();
	};

	payload
		.get(JWT_AUTH_CLAIM)
		.and_then(|v| v.as_object())
		.cloned()
		.unwrap_or_default()
}

#[cfg(test)]
mod tests {
	use base64::engine::general_purpose::URL_SAFE_NO_PAD;

	use super::*;

	fn make_test_jwt(claims: serde_json::Value) -> String {
		let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\",\"typ\":\"JWT\"}");
		let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
		let signature = URL_SAFE_NO_PAD.encode(b"sig");
		format!("{header}.{payload}.{signature}")
	}

	#[test]
	fn parse_id_token_extracts_claims() {
		let claims = serde_json::json!({
			"email": "user@example.com",
			"email_verified": true,
			"https://api.openai.com/auth": {
				"chatgpt_plan_type": "pro",
				"chatgpt_account_id": "acc_123",
				"chatgpt_user_id": "user_456"
			}
		});

		let jwt = make_test_jwt(claims);
		let parsed = parse_id_token(&jwt).unwrap();

		assert_eq!(parsed.email.as_deref(), Some("user@example.com"));
		assert_eq!(parsed.email_verified, Some(true));
		assert_eq!(parsed.plan_type.as_deref(), Some("pro"));
		assert_eq!(parsed.account_id.as_deref(), Some("acc_123"));
		assert_eq!(parsed.user_id.as_deref(), Some("user_456"));
	}

	#[test]
	fn parse_id_token_handles_missing_claims() {
		let claims = serde_json::json!({
			"email": "user@example.com"
		});

		let jwt = make_test_jwt(claims);
		let parsed = parse_id_token(&jwt).unwrap();

		assert_eq!(parsed.email.as_deref(), Some("user@example.com"));
		assert!(parsed.plan_type.is_none());
		assert!(parsed.account_id.is_none());
	}

	#[test]
	fn parse_id_token_rejects_invalid_jwt() {
		let result = parse_id_token("not.a.valid.jwt");
		assert!(result.is_err());

		let result = parse_id_token("only.two");
		assert!(result.is_err());
	}
}
