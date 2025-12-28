//! Error types for configuration parsing.

use std::path::PathBuf;

use thiserror::Error;

/// Errors that can occur when parsing configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
	#[error("KDL parse error: {0}")]
	Kdl(#[from] kdl::KdlError),

	#[error("I/O error reading {path}: {error}")]
	Io {
		path: PathBuf,
		error: std::io::Error,
	},

	#[error("missing required field: {0}")]
	MissingField(String),

	#[error("invalid color format: {0}")]
	InvalidColor(String),

	#[error("invalid modifier: {0}")]
	InvalidModifier(String),

	#[error("invalid theme variant: {0} (expected 'dark' or 'light')")]
	InvalidVariant(String),

	#[error("undefined palette color: ${0}")]
	UndefinedPaletteColor(String),
}

/// Result type for configuration operations.
pub type Result<T> = std::result::Result<T, ConfigError>;
