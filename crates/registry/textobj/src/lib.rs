//! Text objects registry
//!
//! Text objects define selections around semantic units (words, paragraphs,
//! brackets, etc.) with `inner` and `around` variants.
//!
//! This crate provides:
//! - Type definitions ([`TextObjectDef`], [`TextObjectHandler`])
//! - Static registry list ([`TEXT_OBJECTS`])
//! - Registration macros ([`text_object!`], [`symmetric_text_object!`], [`bracket_pair_object!`])
//! - Built-in implementations (word, line, paragraph, surround, quotes, etc.)

use ropey::RopeSlice;
use xeno_primitives::Range;

mod impls;
mod macros;

// Re-export shared types from core registry for consistency
pub use xeno_registry_core::{
	Capability, RegistryEntry, RegistryMeta, RegistryMetadata, RegistrySource, impl_registry_entry,
};
pub use xeno_registry_motions::{flags, movement};

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

/// Registry of all text object definitions.
pub static TEXT_OBJECTS: &[&TextObjectDef] = &[
	&impls::argument::OBJ_argument,
	&impls::line::OBJ_line,
	&impls::number::OBJ_number,
	&impls::paragraph::OBJ_paragraph,
	&impls::quotes::OBJ_double_quotes,
	&impls::quotes::OBJ_single_quotes,
	&impls::quotes::OBJ_backticks,
	&impls::surround::OBJ_parentheses,
	&impls::surround::OBJ_braces,
	&impls::surround::OBJ_brackets,
	&impls::surround::OBJ_angle_brackets,
	&impls::word::OBJ_word,
	&impls::word::OBJ_WORD,
];

/// Finds a text object by trigger character.
pub fn find_by_trigger(trigger: char) -> Option<&'static TextObjectDef> {
	TEXT_OBJECTS
		.iter()
		.copied()
		.find(|o| o.trigger == trigger || o.alt_triggers.contains(&trigger))
}

/// Finds a text object by name or alias.
pub fn find(name: &str) -> Option<&'static TextObjectDef> {
	TEXT_OBJECTS
		.iter()
		.copied()
		.find(|o| o.name() == name || o.aliases().contains(&name))
}

/// Returns all registered text objects.
pub fn all() -> impl Iterator<Item = &'static TextObjectDef> {
	TEXT_OBJECTS.iter().copied()
}
