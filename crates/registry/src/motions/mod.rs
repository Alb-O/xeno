//! Motion registry.
//!
//! Motions are fundamental cursor movement operations (char, word, line, etc.) that
//! actions compose to implement editor commands. Each motion module co-locates its
//! registration with implementation: [`builtins::horizontal`], [`builtins::vertical`],
//! [`builtins::word`], [`builtins::line`], [`builtins::paragraph`], and [`builtins::document`].
//!
//! The [`movement`] module re-exports movement functions and shared utilities.

use ropey::RopeSlice;
use xeno_primitives::Range;
pub use xeno_registry_core::{
	Capability, Key, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta, RegistryMetadata,
	RegistrySource, RuntimeRegistry, impl_registry_entry,
};

#[macro_use]
pub(crate) mod macros;

pub(crate) mod builtins;
pub mod movement;

// Re-export macros
pub use crate::motion;

/// Typed handles for built-in motions.
pub mod keys {
	pub use crate::motions::builtins::diff::*;
	pub use crate::motions::builtins::document::*;
	pub use crate::motions::builtins::horizontal::*;
	pub use crate::motions::builtins::line::*;
	pub use crate::motions::builtins::paragraph::*;
	pub use crate::motions::builtins::vertical::*;
	pub use crate::motions::builtins::word::*;
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

impl_registry_entry!(MotionDef);

/// Typed handle to a motion definition.
pub type MotionKey = Key<MotionDef>;

#[cfg(feature = "db")]
pub use crate::db::MOTIONS;

/// Finds a motion by name or alias.
#[cfg(feature = "db")]
pub fn find(name: &str) -> Option<MotionKey> {
	MOTIONS.get(name).map(MotionKey::new)
}

/// Returns all registered motions, sorted by name.
#[cfg(feature = "db")]
pub fn all() -> impl Iterator<Item = &'static MotionDef> {
	MOTIONS.iter()
}
