//! Text object registry.
//!
//! Text objects define selections around semantic units (words, paragraphs,
//! brackets, etc.) with `inner` and `around` benchmarks.

use ropey::RopeSlice;
use xeno_primitives::Range;

pub mod builtins;
mod macros;

pub use builtins::register_builtins;

pub fn register_plugin(db: &mut crate::db::builder::RegistryDbBuilder) {
	register_builtins(db);
}

inventory::submit! {
	crate::PluginDef::new(
		crate::RegistryMeta::minimal("textobj-builtin", "Text Objects Builtin", "Builtin text object set"),
		register_plugin
	)
}

pub use crate::core::{
	Capability, DuplicatePolicy, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta,
	RegistryMetadata, RegistrySource,
};
pub use crate::motions::{flags, movement};
// Re-export macros
pub use crate::text_object;
pub use crate::{bracket_pair_object, symmetric_text_object};

/// Handler signature for text object selection.
pub type TextObjectHandler = fn(RopeSlice, usize) -> Option<Range>;

/// Definition of a text object.
pub struct TextObjectDef {
	/// Common registry metadata.
	pub meta: RegistryMeta,
	/// Primary trigger character (e.g., 'w' for word).
	pub trigger: char,
	/// Alternative trigger characters.
	pub alt_triggers: &'static [char],
	/// Handler for inner selection mode.
	pub inner: TextObjectHandler,
	/// Handler for around selection mode.
	pub around: TextObjectHandler,
}

impl TextObjectDef {
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

	#[doc(hidden)]
	#[allow(clippy::too_many_arguments, reason = "macro-generated constructor")]
	pub const fn new(
		meta: RegistryMeta,
		trigger: char,
		alt_triggers: &'static [char],
		inner: TextObjectHandler,
		around: TextObjectHandler,
	) -> Self {
		Self {
			meta,
			trigger,
			alt_triggers,
			inner,
			around,
		}
	}
}

crate::impl_registry_entry!(TextObjectDef);

#[cfg(feature = "db")]
pub use crate::db::TEXT_OBJECT_TRIGGER_INDEX;
#[cfg(feature = "db")]
pub use crate::db::TEXT_OBJECTS;

/// Finds a text object by trigger character.
#[cfg(feature = "db")]
pub fn find_by_trigger(trigger: char) -> Option<&'static TextObjectDef> {
	TEXT_OBJECT_TRIGGER_INDEX.get(&trigger).copied()
}

/// Finds a text object by name or alias.
#[cfg(feature = "db")]
pub fn find(name: &str) -> Option<&'static TextObjectDef> {
	TEXT_OBJECTS.get(name)
}

/// Returns all registered text objects, sorted by name.
#[cfg(feature = "db")]
pub fn all() -> impl Iterator<Item = &'static TextObjectDef> {
	TEXT_OBJECTS.iter()
}
