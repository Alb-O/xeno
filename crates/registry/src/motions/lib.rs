//! Motion registry.
//!
//! Motions are fundamental cursor movement operations (char, word, line, etc.) that
//! actions compose to implement editor commands. Each motion module co-locates its
//! registration with implementation: [`horizontal`], [`vertical`], [`word`], [`line`],
//! [`paragraph`], and [`document`].
//!
//! The [`movement`] module re-exports movement functions and shared utilities.

use std::sync::LazyLock;

use ropey::RopeSlice;
use xeno_primitives::Range;
pub use xeno_registry_core::{
	Capability, Key, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta, RegistryMetadata,
	RegistryReg, RegistrySource, impl_registry_entry,
};

#[macro_use]
pub(crate) mod macros;

mod diff;
mod document;
mod horizontal;
mod line;
mod paragraph;
mod vertical;
mod word;

pub mod movement;

/// Registry wrapper for motion definitions.
pub struct MotionReg(pub &'static MotionDef);
inventory::collect!(MotionReg);

impl RegistryReg<MotionDef> for MotionReg {
	fn def(&self) -> &'static MotionDef {
		self.0
	}
}

/// Typed handles for built-in motions.
///
/// Note: Duplicate motion names across crates will conflict at compile time.
pub mod keys {
	pub use crate::motions::diff::*;
	pub use crate::motions::document::*;
	pub use crate::motions::horizontal::*;
	pub use crate::motions::line::*;
	pub use crate::motions::paragraph::*;
	pub use crate::motions::vertical::*;
	pub use crate::motions::word::*;
}

/// Command flags for motion definitions.
pub mod flags {
	/// No flags set.
	pub const NONE: u32 = 0;
}

/// Handler signature for motion primitives.
///
/// # Arguments
///
/// * `text` - The document text as a rope slice
/// * `range` - Current cursor range (anchor..head)
/// * `count` - Repeat count (1 if not specified)
/// * `extend` - Whether to extend selection (vs move cursor)
///
/// Returns the new range after applying the motion.
pub type MotionHandler = fn(RopeSlice, Range, usize, bool) -> Range;

/// Definition of a motion primitive.
///
/// Motions are registered via the [`motion!`] macro and looked up by name
/// from action handlers.
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

/// Indexed collection of all registered motions.
pub static MOTIONS: LazyLock<RegistryIndex<MotionDef>> = LazyLock::new(|| {
	RegistryBuilder::new("motions")
		.extend_inventory::<MotionReg>()
		.sort_by(|a, b| a.name().cmp(b.name()))
		.build()
});

/// Finds a motion by name or alias.
pub fn find(name: &str) -> Option<MotionKey> {
	MOTIONS.get(name).map(MotionKey::new)
}

/// Returns all registered motions, sorted by name.
pub fn all() -> impl Iterator<Item = &'static MotionDef> {
	MOTIONS.iter()
}
