//! Motion registry
//!
//! Motions are the fundamental cursor movement operations (char, word, line, etc.).
//! They're composed by actions to implement editor commands.
//!
//! This crate provides:
//! - Type definitions ([`MotionDef`], [`MotionHandler`])
//! - Distributed slice ([`MOTIONS`])
//! - Registration macro ([`motion!`])
//! - Movement algorithms ([`movement`] module)
//! - Built-in implementations (basic, word, line, document)

use linkme::distributed_slice;
use ropey::RopeSlice;
use xeno_primitives::Range;
pub use xeno_registry_core::{
	Capability, Key, RegistryEntry, RegistryMeta, RegistryMetadata, RegistrySource,
	impl_registry_entry, impl_registry_metadata,
};

/// Built-in motion implementations (char, word, line, etc.).
pub(crate) mod impls;
/// Macro definitions for motion registration.
mod macros;
pub mod movement;

/// Typed handles for built-in motions.
///
/// Note: Duplicate motion names across crates will conflict at compile time.
pub mod keys {
	pub use crate::impls::basic::*;
	pub use crate::impls::document::*;
	pub use crate::impls::line::*;
	pub use crate::impls::paragraph::*;
	pub use crate::impls::word::*;
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
	/// Common registry metadata (id, name, aliases, description, priority, source, caps, flags).
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

/// Registry of all motion definitions.
#[distributed_slice]
pub static MOTIONS: [MotionDef];

/// Finds a motion by name or alias.
pub fn find(name: &str) -> Option<MotionKey> {
	MOTIONS
		.iter()
		.find(|m| m.name() == name || m.aliases().contains(&name))
		.map(MotionKey::new)
}

/// Returns all registered motions.
pub fn all() -> impl Iterator<Item = &'static MotionDef> {
	MOTIONS.iter()
}
