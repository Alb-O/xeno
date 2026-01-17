//! Motion registry with auto-collection via `inventory`.
//!
//! Motions are the fundamental cursor movement operations (char, word, line, etc.).
//! They're composed by actions to implement editor commands.

use std::collections::HashMap;
use std::sync::LazyLock;

use ropey::RopeSlice;
use xeno_primitives::Range;
pub use xeno_registry_core::{
	Capability, Key, RegistryEntry, RegistryMeta, RegistryMetadata, RegistrySource,
	impl_registry_entry,
};

pub(crate) mod impls;
mod macros;
pub mod movement;

/// Wrapper for [`inventory`] collection of motion definitions.
pub struct MotionReg(pub &'static MotionDef);
inventory::collect!(MotionReg);

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

/// O(1) motion lookup index, keyed by name and aliases.
static MOTION_INDEX: LazyLock<HashMap<&'static str, &'static MotionDef>> = LazyLock::new(|| {
	let mut map = HashMap::new();
	for reg in inventory::iter::<MotionReg> {
		let def = reg.0;
		map.insert(def.name(), def);
		for &alias in def.aliases() {
			map.insert(alias, def);
		}
	}
	map
});

/// Lazy reference to all motions for iteration.
pub static MOTIONS: LazyLock<Vec<&'static MotionDef>> = LazyLock::new(|| {
	let mut motions: Vec<_> = inventory::iter::<MotionReg>().map(|r| r.0).collect();
	motions.sort_by_key(|m| m.name());
	motions
});

/// Finds a motion by name or alias.
pub fn find(name: &str) -> Option<MotionKey> {
	MOTION_INDEX.get(name).map(|&def| MotionKey::new(def))
}

/// Returns all registered motions, sorted by name.
pub fn all() -> impl Iterator<Item = &'static MotionDef> {
	MOTIONS.iter().copied()
}
