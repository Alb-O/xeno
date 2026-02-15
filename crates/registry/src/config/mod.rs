//! Configuration types for Xeno.
//!
//! This module provides unified configuration structures that are format-neutral.
//! NUON and Nu script parsing are available behind `config-nuon` and `config-nu`.

use std::collections::HashMap;

#[cfg(feature = "config-nuon")]
pub mod utils;

#[cfg(feature = "config-nuon")]
pub mod nuon;

#[cfg(feature = "config-nu")]
pub mod nu;

#[cfg(feature = "config-nuon")]
pub mod load;

/// Configuration for a language-specific override.
#[derive(Debug, Clone)]
pub struct LanguageConfig {
	/// Language name (e.g., "rust", "python").
	pub name: String,
	/// Option overrides for this language.
	#[cfg(feature = "options")]
	pub options: crate::options::OptionStore,
}

/// Unresolved keybinding configuration (string keys before registry resolution).
#[derive(Debug, Clone, Default)]
pub struct UnresolvedKeys {
	/// Bindings per mode. Key: mode name, Value: key string -> action name.
	pub modes: HashMap<String, HashMap<String, String>>,
}

impl UnresolvedKeys {
	/// Merge another keys config, with `other` taking precedence.
	pub fn merge(&mut self, other: UnresolvedKeys) {
		for (mode, bindings) in other.modes {
			self.modes.entry(mode).or_default().extend(bindings);
		}
	}
}

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
			ConfigWarning::ScopeMismatch { option, found_in, expected } => {
				write!(f, "'{option}' in {found_in} will be ignored (should be in {expected})")
			}
		}
	}
}

/// Configuration error types.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
	/// Error parsing NUON syntax.
	#[cfg(feature = "config-nuon")]
	#[error("NUON parse error: {0}")]
	Nuon(String),

	/// Error parsing Nu script syntax.
	#[cfg(feature = "config-nu")]
	#[error("Nu parse error: {0}")]
	NuParse(String),

	/// Error during Nu script evaluation.
	#[cfg(feature = "config-nu")]
	#[error("Nu runtime error: {0}")]
	NuRuntime(String),

	/// Nu script violated sandbox restrictions.
	#[cfg(feature = "config-nu")]
	#[error("Nu sandbox error: {0}")]
	NuSandbox(String),

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

	/// A field has an invalid type.
	#[error("invalid type for field '{field}': expected {expected}, got {got}")]
	InvalidType {
		/// Field name.
		field: String,
		/// Expected value type.
		expected: &'static str,
		/// Actual value type.
		got: String,
	},

	/// A field name was not recognized.
	#[error("unknown field: {0}")]
	UnknownField(String),
}

/// Result type for configuration operations.
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Parsed configuration from a config file.
///
/// May contain any combination of theme, keys, options, and language settings.
#[derive(Clone, Default)]
pub struct Config {
	/// Parsed theme definition from the config file.
	#[cfg(feature = "themes")]
	pub theme: Option<crate::themes::LinkedThemeDef>,
	/// Keybinding overrides (unresolved strings).
	pub keys: Option<UnresolvedKeys>,
	/// Global option overrides.
	#[cfg(feature = "options")]
	pub options: crate::options::OptionStore,
	/// Per-language option overrides.
	pub languages: Vec<LanguageConfig>,
	/// Non-fatal warnings encountered during parsing.
	pub warnings: Vec<ConfigWarning>,
}

impl std::fmt::Debug for Config {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut s = f.debug_struct("Config");

		#[cfg(feature = "themes")]
		{
			s.field("theme", &self.theme.as_ref().map(|_| "<LinkedThemeDef>"));
		}

		s.field("keys", &self.keys);

		#[cfg(feature = "options")]
		{
			s.field("options", &self.options);
		}

		s.field("languages", &self.languages).field("warnings", &self.warnings).finish()
	}
}

impl Config {
	/// Merge another config into this one.
	///
	/// Values from `other` override values in `self`.
	pub fn merge(&mut self, other: Config) {
		#[cfg(feature = "themes")]
		if other.theme.is_some() {
			self.theme = other.theme;
		}

		if let Some(other_keys) = other.keys {
			match &mut self.keys {
				Some(keys) => keys.merge(other_keys),
				None => self.keys = Some(other_keys),
			}
		}

		#[cfg(feature = "options")]
		self.options.merge(&other.options);

		self.languages.extend(other.languages);
	}
}
