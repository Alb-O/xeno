//! Options registry
//!
//! Options are named settings that can be configured globally or per-buffer.

use std::marker::PhantomData;

pub mod builtins;
pub mod parse;
pub mod registry;
mod resolver;
mod store;
pub mod validators;

pub use builtins::register_builtins;
pub use registry::OptionsRegistry;
pub use resolver::OptionResolver;
pub use store::OptionStore;

/// Typed handles for built-in options.
pub mod keys {
	pub use crate::options::builtins::{
		CURSORLINE, DEFAULT_THEME_ID, SCROLL_LINES, SCROLL_MARGIN, TAB_WIDTH, THEME,
	};
}

pub use crate::core::{
	FromOptionValue, Key, OptionDefault, OptionType, OptionValue, RegistryBuilder, RegistryEntry,
	RegistryIndex, RegistryMeta, RegistryMetadata, RegistrySource, RuntimeRegistry,
};

/// Validator function signature for option constraints.
pub type OptionValidator = fn(&OptionValue) -> Result<(), String>;

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
	/// Common registry metadata.
	pub meta: RegistryMeta,
	/// KDL configuration key (e.g., "tab-width").
	pub kdl_key: &'static str,
	/// Value type constraint.
	pub value_type: OptionType,
	/// Default value factory.
	pub default: OptionDefault,
	/// Application scope.
	pub scope: OptionScope,
	/// Optional validator for value constraints.
	pub validator: Option<OptionValidator>,
}

impl core::fmt::Debug for OptionDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("OptionDef")
			.field("name", &self.meta.name)
			.field("kdl_key", &self.kdl_key)
			.field("value_type", &self.value_type)
			.field("scope", &self.scope)
			.field("priority", &self.meta.priority)
			.field("description", &self.meta.description)
			.finish()
	}
}

/// Typed handle to an option definition.
pub type OptionKey = Key<OptionDef>;

/// Typed handle to an option definition with compile-time type information.
pub struct TypedOptionKey<T: FromOptionValue> {
	inner: OptionKey,
	_marker: PhantomData<T>,
}

impl<T: FromOptionValue> Copy for TypedOptionKey<T> {}

impl<T: FromOptionValue> Clone for TypedOptionKey<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: FromOptionValue> TypedOptionKey<T> {
	/// Creates a new typed option key from a static option definition.
	pub const fn new(def: &'static OptionDef) -> Self {
		Self {
			inner: OptionKey::new(def),
			_marker: PhantomData,
		}
	}

	/// Returns the underlying option definition.
	pub const fn def(self) -> &'static OptionDef {
		self.inner.def()
	}

	/// Returns the untyped option key.
	pub const fn untyped(self) -> OptionKey {
		self.inner
	}
}

impl<T: FromOptionValue> core::fmt::Debug for TypedOptionKey<T> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_tuple("TypedOptionKey")
			.field(&self.inner.def().meta.name)
			.finish()
	}
}

/// Registry wrapper for option definitions.
pub struct OptionReg(pub &'static OptionDef);
inventory::collect!(OptionReg);

#[cfg(feature = "db")]
pub use crate::db::OPTIONS;

/// Finds an option definition by name.
#[cfg(feature = "db")]
pub fn find(name: &str) -> Option<&'static OptionDef> {
	OPTIONS.get(name)
}

/// Finds an option definition by its internal name.
#[cfg(feature = "db")]
pub fn find_by_name(name: &str) -> Option<&'static OptionDef> {
	OPTIONS.get(name)
}

/// Finds an option definition by its KDL configuration key.
#[cfg(feature = "db")]
pub fn find_by_kdl(kdl_key: &str) -> Option<&'static OptionDef> {
	OPTIONS.by_kdl_key(kdl_key)
}

/// Returns all registered options.
#[cfg(feature = "db")]
pub fn all() -> impl Iterator<Item = &'static OptionDef> {
	OPTIONS.iter().into_iter()
}

/// Returns all options sorted by KDL key.
#[cfg(feature = "db")]
pub fn all_sorted() -> impl Iterator<Item = &'static OptionDef> {
	let mut opts: Vec<_> = OPTIONS.items().to_vec();
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

/// Validates that a KDL key exists, the value matches the expected type,
/// and passes any custom validator defined for the option.
#[cfg(feature = "db")]
pub fn validate(kdl_key: &str, value: &OptionValue) -> Result<(), OptionError> {
	let def =
		find_by_kdl(kdl_key).ok_or_else(|| OptionError::UnknownOption(kdl_key.to_string()))?;
	if !value.matches_type(def.value_type) {
		return Err(OptionError::TypeMismatch {
			option: kdl_key.to_string(),
			expected: def.value_type,
			got: value.type_name(),
		});
	}
	if let Some(validator) = def.validator {
		validator(value).map_err(|reason| OptionError::InvalidValue {
			option: kdl_key.to_string(),
			reason,
		})?;
	}
	Ok(())
}

crate::impl_registry_entry!(OptionDef);

use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

inventory::submit! {
	crate::PluginDef::new(
		crate::RegistryMeta::minimal("options-builtin", "Options Builtin", "Builtin option set"),
		register_plugin
	)
}
