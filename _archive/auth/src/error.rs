//! Error types for authentication.

use std::io;

use thiserror::Error;

/// Result type alias for auth operations.
pub type AuthResult<T> = Result<T, AuthError>;

/// Errors that can occur during authentication.
#[derive(Debug, Error)]
pub enum AuthError {
	/// OAuth flow was cancelled by the user.
	#[error("authentication cancelled")]
	Cancelled,

	/// OAuth state parameter mismatch (possible CSRF attack).
	#[error("state mismatch - possible CSRF attack")]
	StateMismatch,

	/// Missing authorization code in callback.
	#[error("missing authorization code")]
	MissingCode,

	/// Token exchange with OAuth server failed.
	#[error("token exchange failed: {0}")]
	TokenExchange(String),

	/// Token refresh failed.
	#[error("token refresh failed: {0}")]
	TokenRefresh(String),

	/// Invalid or malformed token.
	#[error("invalid token: {0}")]
	InvalidToken(String),

	/// Failed to bind to local port for OAuth callback.
	#[error("failed to bind to port {port}: {reason}")]
	PortBinding {
		/// The port that failed to bind.
		port: u16,
		/// The reason for the failure.
		reason: String,
	},

	/// Storage operation failed.
	#[error("storage error: {0}")]
	Storage(String),

	/// Network request failed.
	#[error("network error: {0}")]
	Network(String),

	/// Workspace restriction violation.
	#[error("workspace restriction: {0}")]
	WorkspaceRestriction(String),

	/// Generic I/O error.
	#[error("I/O error: {0}")]
	Io(#[from] io::Error),

	/// Timeout waiting for authentication.
	#[error("authentication timed out")]
	Timeout,

	/// Not authenticated.
	#[error("not authenticated")]
	NotAuthenticated,
}
