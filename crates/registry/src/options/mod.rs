//! Options registry

use std::marker::PhantomData;

pub mod builtins;
pub mod parse;
pub mod registry;
mod resolver;
mod store;
pub mod validators;

pub use builtins::register_builtins;
pub use registry::{OptionsRef, OptionsRegistry};
pub use resolver::OptionResolver;
pub use store::OptionStore;

/// Typed handles for built-in options.
pub mod keys {
	pub use crate::options::builtins::{
		CURSORLINE, DEFAULT_THEME_ID, SCROLL_LINES, SCROLL_MARGIN, TAB_WIDTH, THEME,
	};
}

use crate::core::index::{BuildEntry, RegistryMetaRef};
pub use crate::core::{
	CapabilitySet, FromOptionValue, FrozenInterner, Key, OptionDefault, OptionId, OptionType,
	OptionValue, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta, RegistryMetaStatic,
	RegistryMetadata, RegistrySource, RuntimeRegistry, Symbol, SymbolList,
};

pub type OptionValidator = fn(&OptionValue) -> Result<(), String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionScope {
	Global,
	Buffer,
}

/// Definition of a configurable option (static input).
#[derive(Clone, Copy)]
pub struct OptionDef {
	pub meta: RegistryMetaStatic,
	pub kdl_key: &'static str,
	pub value_type: OptionType,
	pub default: OptionDefault,
	pub scope: OptionScope,
	pub validator: Option<OptionValidator>,
}

impl core::fmt::Debug for OptionDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("OptionDef")
			.field("name", &self.meta.name)
			.field("kdl_key", &self.kdl_key)
			.finish()
	}
}

impl crate::core::RegistryEntry for OptionDef {
	fn meta(&self) -> &RegistryMeta {
		panic!("Called meta() on static OptionDef")
	}
}

/// Symbolized option entry.
pub struct OptionEntry {
	pub meta: RegistryMeta,
	pub kdl_key: Symbol,
	pub value_type: OptionType,
	pub default: OptionDefault,
	pub scope: OptionScope,
	pub validator: Option<OptionValidator>,
}

crate::impl_registry_entry!(OptionEntry);

impl BuildEntry<OptionEntry> for OptionDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			aliases: self.meta.aliases,
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			required_caps: self.meta.required_caps,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		let meta = self.meta_ref();
		sink.push(meta.id);
		sink.push(meta.name);
		sink.push(meta.description);
		for &alias in meta.aliases {
			sink.push(alias);
		}
		sink.push(self.kdl_key);
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> OptionEntry {
		let meta_ref = self.meta_ref();
		let start = alias_pool.len() as u32;
		for &alias in meta_ref.aliases {
			alias_pool.push(interner.get(alias).expect("missing interned alias"));
		}
		// kdl_key acts as an implicit alias for option lookup
		alias_pool.push(
			interner
				.get(self.kdl_key)
				.expect("missing interned kdl_key"),
		);
		let len = (alias_pool.len() as u32 - start) as u16;

		let meta = RegistryMeta {
			id: interner.get(meta_ref.id).expect("missing interned id"),
			name: interner.get(meta_ref.name).expect("missing interned name"),
			description: interner
				.get(meta_ref.description)
				.expect("missing interned description"),
			aliases: SymbolList { start, len },
			priority: meta_ref.priority,
			source: meta_ref.source,
			required_caps: CapabilitySet::from_iter(meta_ref.required_caps.iter().cloned()),
			flags: meta_ref.flags,
		};

		OptionEntry {
			meta,
			kdl_key: interner
				.get(self.kdl_key)
				.expect("missing interned kdl_key"),
			value_type: self.value_type,
			default: self.default,
			scope: self.scope,
			validator: self.validator,
		}
	}
}

/// Handle to an option definition, used for option store lookups.
pub type OptionKey = &'static OptionDef;

/// Typed handle to an option definition with compile-time type information.
pub struct TypedOptionKey<T: FromOptionValue> {
	def: &'static OptionDef,
	_marker: PhantomData<T>,
}

impl<T: FromOptionValue> Clone for TypedOptionKey<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: FromOptionValue> Copy for TypedOptionKey<T> {}

impl<T: FromOptionValue> TypedOptionKey<T> {
	/// Creates a new typed option key from a static definition.
	pub const fn new(def: &'static OptionDef) -> Self {
		Self {
			def,
			_marker: PhantomData,
		}
	}

	/// Returns the underlying option definition.
	pub fn def(&self) -> &'static OptionDef {
		self.def
	}

	/// Returns the KDL key for this option.
	pub fn kdl_key(&self) -> &'static str {
		self.def.kdl_key
	}

	/// Returns the untyped option key for use with [`crate::actions::editor_ctx::OptionAccess::option_raw`].
	pub fn untyped(&self) -> OptionKey {
		self.def
	}
}

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
