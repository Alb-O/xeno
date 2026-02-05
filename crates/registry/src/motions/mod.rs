//! Motion registry.
//!
//! Motions are fundamental cursor movement operations (char, word, line, etc.) that
//! actions compose to implement editor commands. Each motion module co-locates its
//! registration with implementation.
//!
//! The [`movement`] module re-exports movement functions and shared utilities.

use ropey::RopeSlice;
use xeno_primitives::Range;

pub use crate::core::{
	Capability, Key, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta, RegistryMetadata,
	RegistryRef, RegistrySource, RuntimeRegistry,
};

#[macro_use]
pub(crate) mod macros;

pub mod builtins;
pub mod movement;

pub use builtins::register_builtins;

use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

// Re-export macros
pub use crate::motion;

/// Typed handles for built-in motions.
pub mod keys {
	pub use crate::motions::builtins::*;
}

/// Command flags for motion definitions.
pub mod flags {
	/// No flags set.
	pub const NONE: u32 = 0;
}

/// Handler signature for motion primitives.
pub type MotionHandler = fn(RopeSlice, Range, usize, bool) -> Range;

/// Definition of a motion primitive.
pub struct MotionDef {
	/// Common registry metadata.
	pub meta: RegistryMeta,
	/// Function that implements the motion logic.
	pub handler: MotionHandler,
}

impl MotionDef {
	/// Returns the unique identifier.
	pub fn id(&self) -> &'static str {
		self.meta.id
	}

	/// Returns the human-readable name.
	pub fn name(&self) -> &'static str {
		self.meta.name
	}

	/// Returns alternative names for lookup.
	pub fn aliases(&self) -> &'static [&'static str] {
		self.meta.aliases
	}

	/// Returns the description.
	pub fn description(&self) -> &'static str {
		self.meta.description
	}

	/// Returns the priority.
	pub fn priority(&self) -> i16 {
		self.meta.priority
	}

	/// Returns the source.
	pub fn source(&self) -> RegistrySource {
		self.meta.source
	}

	/// Returns required capabilities.
	pub fn required_caps(&self) -> &'static [Capability] {
		self.meta.required_caps
	}

	/// Returns behavior flags.
	pub fn flags(&self) -> u32 {
		self.meta.flags
	}
}

crate::impl_registry_entry!(MotionDef);

/// Typed handle to a motion definition.
pub type MotionKey = Key<MotionDef>;

#[cfg(feature = "db")]
pub use crate::db::MOTIONS;

/// Finds a motion by name or alias.
#[cfg(feature = "db")]
pub fn find(name: &str) -> Option<MotionKey> {
	MOTIONS.get(name).map(MotionKey::new_ref)
}

/// Returns all registered motions, sorted by name.
#[cfg(feature = "db")]
pub fn all() -> Vec<RegistryRef<MotionDef>> {
	let mut items = MOTIONS.all();
	items.sort_by_key(|m| m.name().to_string());
	items
}
