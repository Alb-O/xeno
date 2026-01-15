//! Token types for Claude authentication.

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Stored authentication state for Claude.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthState {
	/// API key (for direct API key auth).
	pub api_key: Option<String>,

	/// OAuth tokens (for Claude Pro/Max subscription auth).
	pub oauth: Option<OAuthTokens>,
}

/// OAuth token set from Claude OAuth flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
	/// Access token for API requests.
	pub access_token: String,

	/// Refresh token for obtaining new access tokens.
	pub refresh_token: String,

	/// Token expiration timestamp (milliseconds since epoch).
	pub expires_at: i64,
}

impl AuthState {
	/// Create auth state with an API key.
	pub fn from_api_key(key: String) -> Self {
		Self {
			api_key: Some(key),
			oauth: None,
		}
	}

	/// Create auth state with OAuth tokens.
	pub fn from_oauth(access: String, refresh: String, expires_at: i64) -> Self {
		Self {
			api_key: None,
			oauth: Some(OAuthTokens {
				access_token: access,
				refresh_token: refresh,
				expires_at,
			}),
		}
	}

	/// Check if authenticated.
	pub fn is_authenticated(&self) -> bool {
		self.api_key.is_some() || self.oauth.is_some()
	}

	/// Check if OAuth tokens are expired.
	pub fn is_expired(&self) -> bool {
		self.oauth
			.as_ref()
			.map(|o| o.expires_at < Utc::now().timestamp_millis())
			.unwrap_or(false)
	}

	/// Get the current access token (API key or OAuth access token).
	pub fn access_token(&self) -> Option<&str> {
		self.api_key
			.as_deref()
			.or(self.oauth.as_ref().map(|o| o.access_token.as_str()))
	}
}
