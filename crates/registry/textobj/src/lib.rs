//! Text objects registry with auto-collection via `inventory`.
//!
//! Text objects define selections around semantic units (words, paragraphs,
//! brackets, etc.) with `inner` and `around` variants.

use std::collections::HashMap;
use std::sync::LazyLock;

use ropey::RopeSlice;
use xeno_primitives::Range;

mod impls;
mod macros;

pub use xeno_registry_core::{
	Capability, DuplicatePolicy, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta,
	RegistryMetadata, RegistryReg, RegistrySource, build_map, impl_registry_entry,
};
pub use xeno_registry_motions::{flags, movement};

/// Wrapper for [`inventory`] collection of text object definitions.
pub struct TextObjectReg(pub &'static TextObjectDef);
inventory::collect!(TextObjectReg);

impl RegistryReg<TextObjectDef> for TextObjectReg {
	fn def(&self) -> &'static TextObjectDef {
		self.0
	}
}

/// Handler signature for text object selection.
///
/// # Arguments
///
/// * `text` - The document text as a rope slice
/// * `pos` - Cursor position (character offset)
///
/// Returns the selected range, or None if no valid selection at position.
pub type TextObjectHandler = fn(RopeSlice, usize) -> Option<Range>;

/// Definition of a text object.
///
/// Text objects have two selection modes:
/// - `inner`: Selects content inside delimiters (e.g., `iw` for inner word)
/// - `around`: Selects content including delimiters (e.g., `aw` for around word)
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

impl_registry_entry!(TextObjectDef);

/// Indexed collection of all text objects with O(1) lookup by name/alias.
pub static TEXT_OBJECTS: LazyLock<RegistryIndex<TextObjectDef>> = LazyLock::new(|| {
	RegistryBuilder::new("text_objects")
		.extend_inventory::<TextObjectReg>()
		.sort_by(|a, b| a.meta.name.cmp(b.meta.name))
		.build()
});

/// O(1) text object lookup by trigger character.
static TEXT_OBJECT_TRIGGER_INDEX: LazyLock<HashMap<char, &'static TextObjectDef>> =
	LazyLock::new(|| {
		let mut map = build_map(
			"textobj.trigger",
			TEXT_OBJECTS.items(),
			DuplicatePolicy::for_build(),
			|def| Some(def.trigger),
		);
		// Also index by alt_triggers
		for def in TEXT_OBJECTS.iter() {
			for &alt in def.alt_triggers {
				map.entry(alt).or_insert(def);
			}
		}
		map
	});

/// Finds a text object by trigger character.
pub fn find_by_trigger(trigger: char) -> Option<&'static TextObjectDef> {
	TEXT_OBJECT_TRIGGER_INDEX.get(&trigger).copied()
}

/// Finds a text object by name or alias.
pub fn find(name: &str) -> Option<&'static TextObjectDef> {
	TEXT_OBJECTS.get(name)
}

/// Returns all registered text objects, sorted by name.
pub fn all() -> impl Iterator<Item = &'static TextObjectDef> {
	TEXT_OBJECTS.iter()
}
