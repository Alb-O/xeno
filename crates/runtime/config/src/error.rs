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

	/// An unknown option was specified in config.
	#[error("unknown option: {key}{}", suggestion.as_ref().map(|s| format!(" (did you mean '{s}'?)")).unwrap_or_default())]
	UnknownOption {
		/// The unrecognized option key.
		key: String,
		/// A suggested alternative, if one is close enough.
		suggestion: Option<String>,
	},

	/// An option value has the wrong type.
	#[error("type mismatch for option '{option}': expected {expected}, got {got}")]
	OptionTypeMismatch {
		/// The option's KDL key.
		option: String,
		/// The expected type name.
		expected: &'static str,
		/// The actual type name.
		got: &'static str,
	},
}

/// Result type for configuration operations.
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Non-fatal warning during configuration parsing.
///
/// These warnings are collected during parsing and reported to the user,
/// but do not prevent the configuration from being loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
	/// An option was used in the wrong scope (e.g., global option in language block).
	ScopeMismatch {
		/// The option's KDL key.
		option: String,
		/// Where the option was found (e.g., "language block").
		found_in: &'static str,
		/// Where the option should be placed (e.g., "global options block").
		expected: &'static str,
	},
}

impl std::fmt::Display for ConfigWarning {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ConfigWarning::ScopeMismatch {
				option,
				found_in,
				expected,
			} => {
				write!(
					f,
					"'{option}' in {found_in} will be ignored (should be in {expected})"
				)
			}
		}
	}
}
