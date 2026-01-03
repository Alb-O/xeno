//! Text objects registry
//!
//! Text objects define selections around semantic units (words, paragraphs,
//! brackets, etc.) with `inner` and `around` variants.
//!
//! This crate provides:
//! - Type definitions ([`TextObjectDef`], [`TextObjectHandler`])
//! - Distributed slice ([`TEXT_OBJECTS`])
//! - Registration macros ([`text_object!`], [`symmetric_text_object!`], [`bracket_pair_object!`])
//! - Standard library implementations (word, line, paragraph, surround, quotes, etc.)

use linkme::distributed_slice;
use ropey::RopeSlice;
use xeno_base::Range;

mod impls;
mod macros;

// Re-export shared types from core registry for consistency
pub use xeno_registry_core::{RegistryMetadata, RegistrySource, impl_registry_metadata};
pub use xeno_registry_motions::{Capability, flags, movement};

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
	/// Unique identifier for this text object.
	pub id: &'static str,
	/// Display name of the text object.
	pub name: &'static str,
	/// Alternative names for lookup.
	pub aliases: &'static [&'static str],
	/// Primary trigger character (e.g., 'w' for word).
	pub trigger: char,
	/// Alternative trigger characters.
	pub alt_triggers: &'static [char],
	/// Human-readable description.
	pub description: &'static str,
	/// Handler for inner selection mode.
	pub inner: TextObjectHandler,
	/// Handler for around selection mode.
	pub around: TextObjectHandler,
	/// Priority for collision resolution.
	pub priority: i16,
	/// Source of this registration.
	pub source: RegistrySource,
	/// Capabilities required to use this text object.
	pub required_caps: &'static [Capability],
	/// Feature flags for this text object.
	pub flags: u32,
}

impl TextObjectDef {
	#[doc(hidden)]
	#[allow(clippy::too_many_arguments, reason = "macro-generated constructor")]
	pub const fn new(
		id: &'static str,
		name: &'static str,
		aliases: &'static [&'static str],
		description: &'static str,
		priority: i16,
		source: RegistrySource,
		required_caps: &'static [Capability],
		flags: u32,
		trigger: char,
		alt_triggers: &'static [char],
		inner: TextObjectHandler,
		around: TextObjectHandler,
	) -> Self {
		Self {
			id,
			name,
			aliases,
			trigger,
			alt_triggers,
			description,
			inner,
			around,
			priority,
			source,
			required_caps,
			flags,
		}
	}
}

/// Registry of all text object definitions.
#[distributed_slice]
pub static TEXT_OBJECTS: [TextObjectDef];

/// Finds a text object by trigger character.
pub fn find_by_trigger(trigger: char) -> Option<&'static TextObjectDef> {
	TEXT_OBJECTS
		.iter()
		.find(|o| o.trigger == trigger || o.alt_triggers.contains(&trigger))
}

/// Finds a text object by name or alias.
pub fn find(name: &str) -> Option<&'static TextObjectDef> {
	TEXT_OBJECTS
		.iter()
		.find(|o| o.name == name || o.aliases.contains(&name))
}

/// Returns all registered text objects.
pub fn all() -> impl Iterator<Item = &'static TextObjectDef> {
	TEXT_OBJECTS.iter()
}

impl_registry_metadata!(TextObjectDef);
