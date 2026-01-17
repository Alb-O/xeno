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
	Capability, RegistryEntry, RegistryMeta, RegistryMetadata, RegistrySource, impl_registry_entry,
};
pub use xeno_registry_motions::{flags, movement};

/// Wrapper for [`inventory`] collection of text object definitions.
pub struct TextObjectReg(pub &'static TextObjectDef);
inventory::collect!(TextObjectReg);

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

/// O(1) text object lookup by name/alias.
static TEXT_OBJECT_NAME_INDEX: LazyLock<HashMap<&'static str, &'static TextObjectDef>> =
	LazyLock::new(|| {
		let mut map = HashMap::new();
		for reg in inventory::iter::<TextObjectReg> {
			let def = reg.0;
			map.insert(def.name(), def);
			for &alias in def.aliases() {
				map.insert(alias, def);
			}
		}
		map
	});

/// O(1) text object lookup by trigger character.
static TEXT_OBJECT_TRIGGER_INDEX: LazyLock<HashMap<char, &'static TextObjectDef>> =
	LazyLock::new(|| {
		let mut map = HashMap::new();
		for reg in inventory::iter::<TextObjectReg> {
			let def = reg.0;
			map.insert(def.trigger, def);
			for &alt in def.alt_triggers {
				map.insert(alt, def);
			}
		}
		map
	});

/// Lazy reference to all text objects for iteration.
pub static TEXT_OBJECTS: LazyLock<Vec<&'static TextObjectDef>> = LazyLock::new(|| {
	let mut objs: Vec<_> = inventory::iter::<TextObjectReg>().map(|r| r.0).collect();
	objs.sort_by_key(|o| o.name());
	objs
});

/// Finds a text object by trigger character.
pub fn find_by_trigger(trigger: char) -> Option<&'static TextObjectDef> {
	TEXT_OBJECT_TRIGGER_INDEX.get(&trigger).copied()
}

/// Finds a text object by name or alias.
pub fn find(name: &str) -> Option<&'static TextObjectDef> {
	TEXT_OBJECT_NAME_INDEX.get(name).copied()
}

/// Returns all registered text objects, sorted by name.
pub fn all() -> impl Iterator<Item = &'static TextObjectDef> {
	TEXT_OBJECTS.iter().copied()
}
