//! Implementation of [`EditorCapabilities`] for [`Editor`].
//!
//! This module provides the [`EditorCaps`] provider, which delegates
//! capability requests to the underlying [`Editor`] instance while
//! maintaining a clean crate boundary.
//!
//! [`EditorCapabilities`]: xeno_registry::EditorCapabilities
//! [`Editor`]: crate::impls::Editor

pub mod command_ops;
pub mod command_queue;
pub mod cursor;
pub mod edit;
pub mod editor_capabilities;
pub mod file_ops;
pub mod focus;
pub mod jump;
pub mod macros;
pub mod mode;
pub mod motion;
pub mod motion_dispatch;
pub mod notification;
pub mod option;
pub mod overlay;
pub mod palette;
pub mod provider;
pub mod search;
pub mod selection;
pub mod split;
pub mod theme;
pub mod undo;
pub mod viewport;

#[cfg(any(test, doc))]
pub(crate) mod invariants;

pub use provider::EditorCaps;
use xeno_registry::commands::CommandError;
use xeno_registry::options::{OptionValue, parse};

/// Parses a string value into an [`OptionValue`] based on the option's declared type.
///
/// Uses centralized validation from the options registry, including type checking
/// and any custom validators defined on the option.
pub(crate) fn parse_option_value(kdl_key: &str, value: &str) -> Result<OptionValue, CommandError> {
	use xeno_registry::options::OptionError;

	parse::parse_value(kdl_key, value).map_err(|e| match e {
		OptionError::UnknownOption(key) => {
			let suggestion = parse::suggest_option(&key);
			match suggestion {
				Some(s) => CommandError::InvalidArgument(format!(
					"unknown option: {key} (did you mean '{s}'?)"
				)),
				None => CommandError::InvalidArgument(format!("unknown option: {key}")),
			}
		}
		OptionError::InvalidValue { option, reason } => {
			CommandError::InvalidArgument(format!("invalid value for {option}: {reason}"))
		}
		OptionError::TypeMismatch {
			option,
			expected,
			got,
		} => CommandError::InvalidArgument(format!(
			"type mismatch for {option}: expected {expected:?}, got {got}"
		)),
	})
}
