//! Error types for configuration parsing.

use std::path::PathBuf;

use thiserror::Error;

/// Errors that can occur when parsing configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
	/// Error parsing KDL syntax.
	#[error("KDL parse error: {0}")]
	Kdl(#[from] kdl::KdlError),

	/// Error reading a configuration file.
	#[error("I/O error reading {path}: {error}")]
	Io {
		/// Path to the file that failed to read.
		path: PathBuf,
		/// The underlying I/O error.
		error: std::io::Error,
	},

	/// A required field is missing from the configuration.
	#[error("missing required field: {0}")]
	MissingField(String),

	/// A color value could not be parsed.
	#[error("invalid color format: {0}")]
	InvalidColor(String),

	/// A style modifier could not be parsed.
	#[error("invalid modifier: {0}")]
	InvalidModifier(String),

	/// A theme variant value is invalid.
	#[error("invalid theme variant: {0} (expected 'dark' or 'light')")]
	InvalidVariant(String),

	/// A palette color reference was not defined.
	#[error("undefined palette color: ${0}")]
	UndefinedPaletteColor(String),

	/// Failed to set up file watching.
	#[error("failed to watch config directory: {0}")]
	Watch(String),
}

/// Result type for configuration operations.
pub type Result<T> = std::result::Result<T, ConfigError>;
