//! Options registry

pub mod builtins;
pub mod def;
pub mod entry;
pub mod parse;
pub mod registry;
mod resolver;
mod store;
pub mod typed_keys;
pub mod validators;

pub use builtins::register_builtins;
pub use def::{OptionDef, OptionInput, OptionScope, OptionValidator};
pub use entry::OptionEntry;
pub use registry::{OptionsRef, OptionsRegistry};
pub use resolver::OptionResolver;
pub use store::OptionStore;
pub use typed_keys::TypedOptionKey;

/// Typed handles for built-in options.
pub mod option_keys {
	pub use crate::options::builtins::{
		CURSORLINE, DEFAULT_THEME_ID, SCROLL_LINES, SCROLL_MARGIN, TAB_WIDTH, THEME,
	};
}

// Re-export for backward compatibility in tests
pub use option_keys as keys;

// Re-exports for convenience and compatibility
pub use crate::core::{
	FromOptionValue, OptionDefault, OptionId, OptionType, OptionValue, RegistryMetaStatic,
	RegistrySource,
};

/// Handle to an option definition, used for option store lookups.
pub type OptionKey = &'static OptionDef;

pub struct OptionReg(pub &'static OptionDef);
inventory::collect!(OptionReg);

#[cfg(feature = "db")]
pub use crate::db::OPTIONS;

#[cfg(feature = "db")]
pub fn find(name: &str) -> Option<OptionsRef> {
	OPTIONS.get(name)
}

#[cfg(feature = "db")]
pub fn all() -> Vec<OptionsRef> {
	OPTIONS.all()
}

/// Validates a parsed option value against the registry definition.
#[cfg(feature = "db")]
pub fn validate(kdl_key: &str, value: &OptionValue) -> Result<(), OptionError> {
	let entry = OPTIONS
		.get(kdl_key)
		.ok_or_else(|| OptionError::UnknownOption(kdl_key.to_string()))?;
	if !value.matches_type(entry.value_type) {
		return Err(OptionError::TypeMismatch {
			option: kdl_key.to_string(),
			expected: entry.value_type,
			got: value.type_name(),
		});
	}
	if let Some(validator) = entry.validator {
		validator(value).map_err(|reason| OptionError::InvalidValue {
			option: kdl_key.to_string(),
			reason,
		})?;
	}
	Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionError {
	UnknownOption(String),
	TypeMismatch {
		option: String,
		expected: OptionType,
		got: &'static str,
	},
	InvalidValue {
		option: String,
		reason: String,
	},
}

impl core::fmt::Display for OptionError {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			OptionError::UnknownOption(key) => write!(f, "unknown option: {key}"),
			OptionError::TypeMismatch {
				option,
				expected,
				got,
			} => {
				write!(
					f,
					"type mismatch for option '{option}': expected {expected:?}, got {got}"
				)
			}
			OptionError::InvalidValue { option, reason } => {
				write!(f, "invalid value for option '{option}': {reason}")
			}
		}
	}
}

impl std::error::Error for OptionError {}
