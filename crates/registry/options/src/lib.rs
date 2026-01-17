//! Options registry
//!
//! Options are named settings that can be configured globally or per-buffer.
//! This crate provides:
//! - Type definitions ([`OptionDef`], [`OptionValue`], [`OptionType`], [`OptionScope`])
//! - Registry list ([`OPTIONS`])
//! - Registration via `#[derive_option]` proc macro
//! - Typed keys ([`TypedOptionKey<T>`]) for compile-time type safety
//! - Validation via [`OptionDef::validator`] and [`validators`] module
//!
//! # Available Options
//!
//! | Option | Type | Scope | Default | Description |
//! |--------|------|-------|---------|-------------|
//! | `tab-width` | int | buffer | 4 | Spaces per tab character |
//! | `theme` | string | global | "gruvbox" | Color theme name |
//!
//! # Defining Options
//!
//! ```ignore
//! use xeno_macro::derive_option;
//!
//! #[derive_option]
//! #[option(kdl = "tab-width", scope = buffer)]
//! /// Number of spaces a tab character occupies for display.
//! pub static TAB_WIDTH: i64 = 4;
//! ```
//!
//! # Accessing Options
//!
//! Options are accessed via typed keys for compile-time safety:
//!
//! ```ignore
//! use xeno_registry_options::keys;
//!
//! // Type-safe access via TypedOptionKey (resolves buffer -> language -> global -> default)
//! let width: i64 = ctx.option(keys::TAB_WIDTH);
//!
//! // Buffer-level access with resolution chain
//! let tab_width: i64 = buffer.option(keys::TAB_WIDTH, &editor);
//!
//! // Editor-level access for focused buffer
//! let theme: String = editor.option(keys::THEME);
//! ```
//!
//! # Validation
//!
//! Options can define custom validators via the [`OptionDef::validator`] field.
//! The [`validators`] module provides common validators:
//!
//! ```ignore
//! use xeno_registry_options::validators;
//!
//! // Validates that TAB_WIDTH is >= 1
//! validators::positive_int(&OptionValue::Int(4)); // Ok(())
//! validators::positive_int(&OptionValue::Int(0)); // Err("must be at least 1")
//! ```
//!
//! # Config Loading
//!
//! Options are loaded from `~/.config/xeno/config.kdl` at startup:
//!
//! ```kdl
//! options {
//!     tab-width 4
//!     theme "gruvbox"
//! }
//!
//! language "rust" {
//!     tab-width 2  // Buffer-scoped options can be per-language
//! }
//! ```
//!
//! Resolution order: Buffer-local → Language config → Global config → Compile-time default
//!
//! # Scope Validation
//!
//! Options have a scope (global or buffer). Global options (like `theme`) in
//! language blocks will generate warnings at parse time and be ignored.
use std::marker::PhantomData;
use std::sync::LazyLock;

// Re-export self for proc macro absolute path resolution
#[doc(hidden)]
pub extern crate self as xeno_registry_options;

mod impls;
pub mod parse;
mod resolver;
mod store;
pub mod validators;

pub use resolver::OptionResolver;
pub use store::OptionStore;

/// Typed handles for built-in options.
///
/// Use these constants to reference options in a type-safe manner:
///
/// ```ignore
/// use xeno_registry_options::keys;
///
/// // TypedOptionKey<i64> provides compile-time type safety
/// let def = keys::TAB_WIDTH.def();
/// println!("Default tab width: {:?}", (def.default)());
/// ```
pub mod keys {
	pub use crate::impls::cursorline::*;
	pub use crate::impls::indent::*;
	pub use crate::impls::scroll::*;
	pub use crate::impls::theme::*;
}

pub use xeno_registry_core::{
	FromOptionValue, Key, OptionType, OptionValue, RegistryBuilder, RegistryEntry, RegistryIndex,
	RegistryMeta, RegistryMetadata, RegistryReg, RegistrySource, impl_registry_entry,
};

/// Wrapper for [`inventory`] collection of option definitions.
pub struct OptionReg(pub &'static OptionDef);
inventory::collect!(OptionReg);

impl RegistryReg<OptionDef> for OptionReg {
	fn def(&self) -> &'static OptionDef {
		self.0
	}
}

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
	///
	/// This is the exact string that appears in config files - no automatic
	/// transformation between snake_case and kebab-case.
	pub kdl_key: &'static str,
	/// Value type constraint.
	pub value_type: OptionType,
	/// Default value factory.
	pub default: fn() -> OptionValue,
	/// Application scope.
	pub scope: OptionScope,
	/// Optional validator for value constraints.
	///
	/// Returns `Ok(())` if valid, `Err(reason)` if invalid.
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
/// let value = get_default(keys::TAB_WIDTH.untyped());
/// ```
pub type OptionKey = Key<OptionDef>;

/// Typed handle to an option definition with compile-time type information.
///
/// Unlike [`OptionKey`], this wrapper carries the Rust type `T` at compile time,
/// enabling type-safe option access without runtime type checking.
///
/// # Example
///
/// ```ignore
/// use xeno_registry_options::{keys, TypedOptionKey};
///
/// // keys::TAB_WIDTH is TypedOptionKey<i64>
/// let width: i64 = ctx.option(keys::TAB_WIDTH);
/// ```
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

/// Indexed collection of all options.
pub static OPTIONS: LazyLock<RegistryIndex<OptionDef>> = LazyLock::new(|| {
	RegistryBuilder::new("options")
		.extend_inventory::<OptionReg>()
		.sort_by(|a, b| a.meta.priority.cmp(&b.meta.priority))
		.build()
});

/// O(1) option lookup index by KDL key.
static OPTION_KDL_INDEX: LazyLock<std::collections::HashMap<&'static str, &'static OptionDef>> =
	LazyLock::new(|| {
		let mut map = std::collections::HashMap::new();
		for opt in OPTIONS.iter() {
			map.insert(opt.kdl_key, opt);
		}
		map
	});

/// Finds an option definition by name.
pub fn find(name: &str) -> Option<&'static OptionDef> {
	OPTIONS.get(name)
}

/// Finds an option definition by its internal name.
///
/// This is equivalent to [`find`] and is provided for clarity when
/// distinguishing between name-based and KDL key-based lookups.
pub fn find_by_name(name: &str) -> Option<&'static OptionDef> {
	OPTIONS.get(name)
}

/// Finds an option definition by its KDL configuration key.
///
/// Use this when parsing config files where options are identified
/// by their KDL key (e.g., "tab-width" instead of "tab_width").
pub fn find_by_kdl(kdl_key: &str) -> Option<&'static OptionDef> {
	OPTION_KDL_INDEX.get(kdl_key).copied()
}

/// Returns all registered options.
pub fn all() -> impl Iterator<Item = &'static OptionDef> {
	OPTIONS.iter()
}

/// Returns all options sorted by KDL key.
///
/// Useful for documentation, completion, and consistent ordering.
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
///
/// Returns `Ok(())` if valid, or an appropriate [`OptionError`] if not.
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

impl_registry_entry!(OptionDef);
