use thiserror::Error;

/// Errors specific to the notification system.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NotificationError {
	/// Invalid configuration provided.
	#[error("Invalid configuration: {0}")]
	InvalidConfig(String),

	/// Content exceeds size limits.
	#[error("Content too large: {0} bytes exceeds limit of {1} bytes")]
	ContentTooLarge(usize, usize),
}
