//! Shared registry infrastructure.
//!
//! This crate provides foundational types for the registry system:
//! - [`ActionId`]: Numeric identifier for actions
//! - [`RegistrySource`]: Where a registry item was defined
//! - [`RegistryMeta`]: Common metadata struct for registry items
//! - [`RegistryEntry`]: Trait for accessing registry metadata
//! - [`Capability`]: Editor capability requirements
//! - [`CommandError`]: Errors from command/action execution
//! - [`Key`]: Typed handle to a registry definition

use thiserror::Error;

/// Represents an editor capability required by a registry item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
	/// Read access to document text.
	Text,
	/// Access to cursor position.
	Cursor,
	/// Access to selection state.
	Selection,
	/// Access to editor mode (normal, insert, visual).
	Mode,
	/// Ability to display messages and notifications.
	Messaging,
	/// Ability to modify document text.
	Edit,
	/// Access to search functionality.
	Search,
	/// Access to undo/redo history.
	Undo,
	/// Access to file system operations.
	FileOps,
}

/// Errors that can occur during command execution.
///
/// This error type is shared between the command and action registries to avoid
/// circular dependencies. Actions re-export this type for convenience.
#[derive(Error, Debug, Clone)]
pub enum CommandError {
	/// General command failure with message.
	#[error("{0}")]
	Failed(String),
	/// A required argument was not provided.
	#[error("missing argument: {0}")]
	MissingArgument(&'static str),
	/// An argument was provided but invalid.
	#[error("invalid argument: {0}")]
	InvalidArgument(String),
	/// File I/O operation failed.
	#[error("I/O error: {0}")]
	Io(String),
	/// Command name was not found in registry.
	#[error("command not found: {0}")]
	NotFound(String),
	/// Command requires a capability the context doesn't provide.
	#[error("missing capability: {0:?}")]
	MissingCapability(Capability),
	/// Operation not supported in current context.
	#[error("unsupported operation: {0}")]
	Unsupported(&'static str),
	/// Catch-all for other errors.
	#[error("{0}")]
	Other(String),
}

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

// Seal the FromOptionValue trait to prevent external implementations.
mod sealed {
	pub trait Sealed {}
	impl Sealed for i64 {}
	impl Sealed for bool {}
	impl Sealed for String {}
}

/// Trait for types that can be extracted from an [`OptionValue`].
///
/// This trait is sealed and only implemented for:
/// - `i64` (from `OptionValue::Int`)
/// - `bool` (from `OptionValue::Bool`)
/// - `String` (from `OptionValue::String`)
pub trait FromOptionValue: sealed::Sealed + Sized {
	/// Extracts the value from an `OptionValue`, returning `None` if the type doesn't match.
	fn from_option(value: &OptionValue) -> Option<Self>;

	/// Returns the `OptionType` corresponding to this Rust type.
	fn option_type() -> OptionType;
}

impl FromOptionValue for i64 {
	fn from_option(value: &OptionValue) -> Option<Self> {
		value.as_int()
	}

	fn option_type() -> OptionType {
		OptionType::Int
	}
}

impl FromOptionValue for bool {
	fn from_option(value: &OptionValue) -> Option<Self> {
		value.as_bool()
	}

	fn option_type() -> OptionType {
		OptionType::Bool
	}
}

impl FromOptionValue for String {
	fn from_option(value: &OptionValue) -> Option<Self> {
		value.as_str().map(|s| s.to_string())
	}

	fn option_type() -> OptionType {
		OptionType::String
	}
}

/// Numeric identifier for an action in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionId(pub u32);

impl ActionId {
	/// Represents an invalid action ID.
	pub const INVALID: ActionId = ActionId(u32::MAX);

	/// Returns true if this action ID is valid.
	#[inline]
	pub fn is_valid(self) -> bool {
		self != Self::INVALID
	}

	/// Returns the underlying u32 value.
	#[inline]
	pub fn as_u32(self) -> u32 {
		self.0
	}
}

impl std::fmt::Display for ActionId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if *self == Self::INVALID {
			write!(f, "ActionId(INVALID)")
		} else {
			write!(f, "ActionId({})", self.0)
		}
	}
}

/// Represents where a registry item was defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegistrySource {
	/// Built directly into the editor.
	Builtin,
	/// Defined in a library crate.
	Crate(&'static str),
	/// Loaded at runtime (e.g., from KDL config files).
	Runtime,
}

impl core::fmt::Display for RegistrySource {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Builtin => write!(f, "builtin"),
			Self::Crate(name) => write!(f, "crate:{name}"),
			Self::Runtime => write!(f, "runtime"),
		}
	}
}

/// Common metadata for all registry item types.
///
/// This struct consolidates the standard fields shared across all registry
/// definitions (actions, motions, commands, text objects, etc.), reducing
/// boilerplate and enabling generic introspection.
///
/// # Fields
///
/// All registry items have these properties:
/// - `id`: Unique identifier (typically `"crate::name"`)
/// - `name`: Human-readable display name
/// - `aliases`: Alternative names for lookup
/// - `description`: Help text description
/// - `priority`: Collision resolution (higher wins)
/// - `source`: Origin (builtin, crate, runtime)
/// - `required_caps`: Capabilities needed to execute
/// - `flags`: Bitflags for behavior hints
#[derive(Debug, Clone, Copy)]
pub struct RegistryMeta {
	/// Unique identifier (e.g., "xeno-stdlib::move_left").
	pub id: &'static str,
	/// Human-readable name for UI display.
	pub name: &'static str,
	/// Alternative names for command/action lookup.
	pub aliases: &'static [&'static str],
	/// Description for help text.
	pub description: &'static str,
	/// Priority for conflict resolution (higher wins).
	pub priority: i16,
	/// Where this item was defined.
	pub source: RegistrySource,
	/// Capabilities required to execute this item.
	pub required_caps: &'static [Capability],
	/// Bitflags for additional behavior hints.
	pub flags: u32,
}

impl RegistryMeta {
	/// Creates a new RegistryMeta with all fields specified.
	#[allow(clippy::too_many_arguments, reason = "constructor for all fields")]
	pub const fn new(
		id: &'static str,
		name: &'static str,
		aliases: &'static [&'static str],
		description: &'static str,
		priority: i16,
		source: RegistrySource,
		required_caps: &'static [Capability],
		flags: u32,
	) -> Self {
		Self {
			id,
			name,
			aliases,
			description,
			priority,
			source,
			required_caps,
			flags,
		}
	}

	/// Creates a minimal RegistryMeta with defaults for optional fields.
	pub const fn minimal(id: &'static str, name: &'static str, description: &'static str) -> Self {
		Self {
			id,
			name,
			aliases: &[],
			description,
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		}
	}
}

/// Trait for accessing registry metadata from definition types.
///
/// Implement this trait to enable generic registry operations like
/// collision detection, help generation, and introspection.
pub trait RegistryEntry {
	/// Returns the metadata struct for this registry item.
	fn meta(&self) -> &RegistryMeta;

	/// Returns the unique identifier.
	fn id(&self) -> &'static str {
		self.meta().id
	}

	/// Returns the human-readable name.
	fn name(&self) -> &'static str {
		self.meta().name
	}

	/// Returns alternative names for lookup.
	fn aliases(&self) -> &'static [&'static str] {
		self.meta().aliases
	}

	/// Returns the description.
	fn description(&self) -> &'static str {
		self.meta().description
	}

	/// Returns the priority for collision resolution.
	fn priority(&self) -> i16 {
		self.meta().priority
	}

	/// Returns where this item was defined.
	fn source(&self) -> RegistrySource {
		self.meta().source
	}

	/// Returns capabilities required to execute this item.
	fn required_caps(&self) -> &'static [Capability] {
		self.meta().required_caps
	}

	/// Returns behavior flags.
	fn flags(&self) -> u32 {
		self.meta().flags
	}
}

/// Trait for basic metadata access.
///
/// This trait provides the minimal metadata interface. Types implementing
/// [`RegistryEntry`] (with `meta: RegistryMeta` field) get this automatically
/// via [`impl_registry_entry!`].
pub trait RegistryMetadata {
	/// Returns the unique identifier for this registry item.
	fn id(&self) -> &'static str;
	/// Returns the human-readable name for this registry item.
	fn name(&self) -> &'static str;
	/// Returns the priority for collision resolution (higher wins).
	fn priority(&self) -> i16;
	/// Returns where this registry item was defined.
	fn source(&self) -> RegistrySource;
}

/// Implements [`RegistryEntry`] and [`RegistryMetadata`] for a type with a `meta: RegistryMeta` field.
#[macro_export]
macro_rules! impl_registry_entry {
	($type:ty) => {
		impl $crate::RegistryEntry for $type {
			fn meta(&self) -> &$crate::RegistryMeta {
				&self.meta
			}
		}

		impl $crate::RegistryMetadata for $type {
			fn id(&self) -> &'static str {
				self.meta.id
			}
			fn name(&self) -> &'static str {
				self.meta.name
			}
			fn priority(&self) -> i16 {
				self.meta.priority
			}
			fn source(&self) -> $crate::RegistrySource {
				self.meta.source
			}
		}
	};
}

/// Typed handle to a registry definition.
///
/// Zero-cost wrapper around a static reference. Provides compile-time
/// safety for internal registry references.
pub struct Key<T: 'static>(&'static T);

impl<T: 'static> Copy for Key<T> {}

impl<T: 'static> Clone for Key<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T> Key<T> {
	/// Creates a new typed handle from a static reference.
	pub const fn new(def: &'static T) -> Self {
		Self(def)
	}

	/// Returns the underlying definition.
	pub const fn def(self) -> &'static T {
		self.0
	}
}

impl<T: RegistryMetadata> Key<T> {
	/// Returns the name of the referenced definition.
	pub fn name(self) -> &'static str {
		self.0.name()
	}
}

impl<T: RegistryMetadata> core::fmt::Debug for Key<T> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_tuple("Key").field(&self.0.name()).finish()
	}
}
