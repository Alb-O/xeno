use super::meta::{RegistryMeta, RegistrySource};
use super::symbol::Symbol;

/// Trait for accessing registry metadata from definition types.
pub trait RegistryEntry {
	/// Returns the metadata struct for this registry item.
	fn meta(&self) -> &RegistryMeta;

	/// Returns the unique identifier.
	fn id(&self) -> Symbol {
		self.meta().id
	}

	/// Returns the human-readable name.
	fn name(&self) -> Symbol {
		self.meta().name
	}

	/// Returns the description.
	fn description(&self) -> Symbol {
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

	/// Returns whether this item mutates buffer text.
	fn mutates_buffer(&self) -> bool {
		self.meta().mutates_buffer
	}

	/// Returns behavior flags.
	fn flags(&self) -> u32 {
		self.meta().flags
	}
}

/// Trait for basic metadata access.
pub trait RegistryMetadata {
	/// Returns the unique identifier for this registry item.
	fn id(&self) -> Symbol;
	/// Returns the human-readable name for this registry item.
	fn name(&self) -> Symbol;
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
			fn id(&self) -> $crate::Symbol {
				self.meta.id
			}
			fn name(&self) -> $crate::Symbol {
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

/// Selects a provided value or falls back to a default.
#[doc(hidden)]
#[macro_export]
macro_rules! __reg_opt {
	({$val:expr}, $default:expr) => {
		$val
	};
	(, $default:expr) => {
		$default
	};
}

/// Selects a provided slice or returns an empty slice.
#[doc(hidden)]
#[macro_export]
macro_rules! __reg_opt_slice {
	({$val:expr}) => {
		$val
	};
	() => {
		&[]
	};
}
