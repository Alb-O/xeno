//! Options registry
//!
//! Options are named settings that can be configured globally or per-buffer.
//! This crate provides:
//! - Type definitions ([`OptionDef`], [`OptionValue`], [`OptionType`], [`OptionScope`])
//! - Distributed slice ([`OPTIONS`])
//! - Registration macro ([`option!`])
//! - Standard library implementations (indent, display, behavior, etc.)

use std::sync::OnceLock;

use linkme::distributed_slice;

mod impls;
mod macros;
mod resolver;
mod store;

/// Global options storage, initialized from user config at startup.
static GLOBAL_OPTIONS: OnceLock<OptionStore> = OnceLock::new();

pub use resolver::OptionResolver;
pub use store::OptionStore;

/// Typed handles for built-in options.
///
/// Use these constants to reference options in a type-safe manner:
///
/// ```ignore
/// use xeno_registry_options::keys;
///
/// let def = keys::tab_width.def();
/// println!("Default tab width: {:?}", (def.default)());
/// ```
pub mod keys {
	pub use crate::impls::behavior::*;
	pub use crate::impls::display::*;
	pub use crate::impls::file::*;
	pub use crate::impls::indent::*;
	pub use crate::impls::scroll::*;
	pub use crate::impls::search::*;
	pub use crate::impls::theme::*;
}

pub use xeno_registry_core::{Key, RegistryMetadata, RegistrySource, impl_registry_metadata};

/// The value of an option.
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
	/// Boolean value (true/false).
	Bool(bool),
	/// Integer value.
	Int(i64),
	/// String value.
	String(String),
}

impl OptionValue {
	/// Returns the boolean value if this is a `Bool` variant.
	pub fn as_bool(&self) -> Option<bool> {
		match self {
			OptionValue::Bool(v) => Some(*v),
			_ => None,
		}
	}

	/// Returns the integer value if this is an `Int` variant.
	pub fn as_int(&self) -> Option<i64> {
		match self {
			OptionValue::Int(v) => Some(*v),
			_ => None,
		}
	}

	/// Returns the string value if this is a `String` variant.
	pub fn as_str(&self) -> Option<&str> {
		match self {
			OptionValue::String(v) => Some(v),
			_ => None,
		}
	}

	/// Returns true if this value matches the given type.
	pub fn matches_type(&self, ty: OptionType) -> bool {
		matches!(
			(self, ty),
			(OptionValue::Bool(_), OptionType::Bool)
				| (OptionValue::Int(_), OptionType::Int)
				| (OptionValue::String(_), OptionType::String)
		)
	}

	/// Returns the type name of this value.
	pub fn type_name(&self) -> &'static str {
		match self {
			OptionValue::Bool(_) => "bool",
			OptionValue::Int(_) => "int",
			OptionValue::String(_) => "string",
		}
	}
}

impl From<bool> for OptionValue {
	fn from(v: bool) -> Self {
		OptionValue::Bool(v)
	}
}

impl From<i64> for OptionValue {
	fn from(v: i64) -> Self {
		OptionValue::Int(v)
	}
}

impl From<String> for OptionValue {
	fn from(v: String) -> Self {
		OptionValue::String(v)
	}
}

impl From<&str> for OptionValue {
	fn from(v: &str) -> Self {
		OptionValue::String(v.to_string())
	}
}

/// The type of an option's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
	/// Boolean type.
	Bool,
	/// Integer type.
	Int,
	/// String type.
	String,
}

/// Scope for option application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionScope {
	/// Global option (applies to all buffers).
	Global,
	/// Buffer-local option (can be overridden per-buffer).
	Buffer,
}

/// Definition of a configurable option.
pub struct OptionDef {
	/// Unique identifier (e.g., "xeno_registry_options::tab_width").
	pub id: &'static str,
	/// Internal name for typed key references (e.g., "tab_width").
	pub name: &'static str,
	/// KDL configuration key (e.g., "tab-width").
	///
	/// This is the exact string that appears in config files - no automatic
	/// transformation between snake_case and kebab-case.
	pub kdl_key: &'static str,
	/// Human-readable description.
	pub description: &'static str,
	/// Value type constraint.
	pub value_type: OptionType,
	/// Default value factory.
	pub default: fn() -> OptionValue,
	/// Application scope.
	pub scope: OptionScope,
	/// Priority for ordering (documentation, completion).
	pub priority: i16,
	/// Origin of definition.
	pub source: RegistrySource,
}

impl core::fmt::Debug for OptionDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("OptionDef")
			.field("name", &self.name)
			.field("kdl_key", &self.kdl_key)
			.field("value_type", &self.value_type)
			.field("scope", &self.scope)
			.field("priority", &self.priority)
			.field("description", &self.description)
			.finish()
	}
}

/// Typed handle to an option definition.
///
/// Use this instead of raw string lookups for compile-time safety:
///
/// ```ignore
/// use xeno_registry_options::{keys, OptionKey};
///
/// fn get_default(key: OptionKey) -> OptionValue {
///     (key.def().default)()
/// }
///
/// let value = get_default(keys::tab_width);
/// ```
pub type OptionKey = Key<OptionDef>;

/// Registry of all option definitions.
#[distributed_slice]
pub static OPTIONS: [OptionDef];

/// Finds an option definition by name.
pub fn find(name: &str) -> Option<&'static OptionDef> {
	OPTIONS.iter().find(|o| o.name == name)
}

/// Finds an option definition by its internal name.
///
/// This is equivalent to [`find`] and is provided for clarity when
/// distinguishing between name-based and KDL key-based lookups.
pub fn find_by_name(name: &str) -> Option<&'static OptionDef> {
	OPTIONS.iter().find(|o| o.name == name)
}

/// Finds an option definition by its KDL configuration key.
///
/// Use this when parsing config files where options are identified
/// by their KDL key (e.g., "tab-width" instead of "tab_width").
pub fn find_by_kdl(kdl_key: &str) -> Option<&'static OptionDef> {
	OPTIONS.iter().find(|o| o.kdl_key == kdl_key)
}

/// Returns all registered options.
pub fn all() -> impl Iterator<Item = &'static OptionDef> {
	OPTIONS.iter()
}

/// Returns all options sorted by KDL key.
///
/// Useful for documentation, completion, and consistent ordering.
pub fn all_sorted() -> impl Iterator<Item = &'static OptionDef> {
	let mut opts: Vec<_> = OPTIONS.iter().collect();
	opts.sort_by_key(|o| o.kdl_key);
	opts.into_iter()
}

/// Error type for option validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionError {
	/// The option KDL key is not recognized.
	UnknownOption(String),
	/// The value type does not match the option's declared type.
	TypeMismatch {
		/// The option's KDL key.
		option: String,
		/// The expected type.
		expected: OptionType,
		/// The actual type name of the provided value.
		got: &'static str,
	},
	/// The value fails validation constraints.
	InvalidValue {
		/// The option's KDL key.
		option: String,
		/// Human-readable reason for validation failure.
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
				write!(f, "type mismatch for option '{option}': expected {expected:?}, got {got}")
			}
			OptionError::InvalidValue { option, reason } => {
				write!(f, "invalid value for option '{option}': {reason}")
			}
		}
	}
}

impl std::error::Error for OptionError {}

/// Initialize the global options store from user configuration.
///
/// This should be called once at startup after parsing the config file.
/// Subsequent calls are no-ops (the first store wins).
///
/// # Example
///
/// ```ignore
/// use xeno_registry_options::{init_global, OptionStore};
///
/// let store = parse_config_options(config)?;
/// init_global(store);
/// ```
pub fn init_global(store: OptionStore) {
	let _ = GLOBAL_OPTIONS.set(store);
}

/// Get a global option value, ignoring buffer-local overrides.
///
/// This resolves directly against the global store (or returns the
/// compile-time default if not set). Use this for options that are
/// inherently global, like `theme`.
///
/// For context-aware resolution that respects buffer-local overrides,
/// use [`Buffer::option()`] or [`OptionResolver`] instead.
///
/// # Example
///
/// ```ignore
/// use xeno_registry_options::{global, keys};
///
/// let theme = global(keys::theme);
/// ```
pub fn global(key: OptionKey) -> OptionValue {
	GLOBAL_OPTIONS
		.get()
		.and_then(|s| s.get(key).cloned())
		.unwrap_or_else(|| (key.def().default)())
}

/// Validates that a KDL key exists and the value matches the expected type.
///
/// Returns `Ok(())` if valid, or an appropriate [`OptionError`] if not.
pub fn validate(kdl_key: &str, value: &OptionValue) -> Result<(), OptionError> {
	let def = find_by_kdl(kdl_key).ok_or_else(|| OptionError::UnknownOption(kdl_key.to_string()))?;
	if !value.matches_type(def.value_type) {
		return Err(OptionError::TypeMismatch {
			option: kdl_key.to_string(),
			expected: def.value_type,
			got: value.type_name(),
		});
	}
	Ok(())
}

impl_registry_metadata!(OptionDef);
