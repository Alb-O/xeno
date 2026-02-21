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
}

/// Implements [`RegistryEntry`] for a type with a `meta: RegistryMeta` field.
#[macro_export]
macro_rules! impl_registry_entry {
	($type:ty) => {
		impl $crate::RegistryEntry for $type {
			fn meta(&self) -> &$crate::RegistryMeta {
				&self.meta
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
