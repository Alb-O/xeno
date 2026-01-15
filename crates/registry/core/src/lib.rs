//! Shared registry infrastructure.
//!
//! This crate provides foundational types for the registry system:
//! - [`ActionId`]: Numeric identifier for actions
//! - [`RegistrySource`]: Where a registry item was defined
//! - [`RegistryMetadata`]: Common metadata trait for registry items
//! - [`Key`]: Typed handle to a registry definition

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
/// Implemented by each registry definition type to enable generic
/// operations like collision detection and diagnostics.
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

/// Implements [`RegistryMetadata`] for a type with `id`, `name`, `priority`, and `source` fields.
#[macro_export]
macro_rules! impl_registry_metadata {
	($type:ty) => {
		impl $crate::RegistryMetadata for $type {
			fn id(&self) -> &'static str {
				self.id
			}
			fn name(&self) -> &'static str {
				self.name
			}
			fn priority(&self) -> i16 {
				self.priority
			}
			fn source(&self) -> $crate::RegistrySource {
				self.source
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
