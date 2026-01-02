//! Motion registry for Evildoer editor.
//!
//! Motions are the fundamental cursor movement operations (char, word, line, etc.).
//! They're composed by actions to implement editor commands.
//!
//! This crate provides:
//! - Type definitions ([`MotionDef`], [`MotionHandler`])
//! - Distributed slice ([`MOTIONS`])
//! - Registration macro ([`motion!`])
//! - Movement algorithms ([`movement`] module)
//! - Standard library implementations (basic, word, line, document)

use evildoer_base::Range;
use linkme::distributed_slice;
use ropey::RopeSlice;

mod impls;
mod macros;
pub mod movement;

/// Represents where a registry item was defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegistrySource {
	/// Built directly into the editor.
	Builtin,
	/// Defined in a library crate.
	Crate(&'static str),
	/// Loaded at runtime (e.g., from KDL config files).
	Runtime,
}

impl core::fmt::Display for RegistrySource {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Builtin => write!(f, "builtin"),
			Self::Crate(name) => write!(f, "crate:{name}"),
			Self::Runtime => write!(f, "runtime"),
		}
	}
}

/// Represents an editor capability required by a registry item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
	Text,
	Cursor,
	Selection,
	Mode,
	Messaging,
	Edit,
	Search,
	Undo,
	FileOps,
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
	pub id: &'static str,
	pub name: &'static str,
	pub aliases: &'static [&'static str],
	pub description: &'static str,
	pub handler: MotionHandler,
	pub priority: i16,
	pub source: RegistrySource,
	pub required_caps: &'static [Capability],
	pub flags: u32,
}

impl MotionDef {
	#[doc(hidden)]
	#[allow(clippy::too_many_arguments)]
	pub const fn new(
		id: &'static str,
		name: &'static str,
		aliases: &'static [&'static str],
		description: &'static str,
		priority: i16,
		source: RegistrySource,
		required_caps: &'static [Capability],
		flags: u32,
		handler: MotionHandler,
	) -> Self {
		Self {
			id,
			name,
			aliases,
			description,
			handler,
			priority,
			source,
			required_caps,
			flags,
		}
	}
}

/// Registry of all motion definitions.
#[distributed_slice]
pub static MOTIONS: [MotionDef];

/// Finds a motion by name or alias.
pub fn find(name: &str) -> Option<&'static MotionDef> {
	MOTIONS
		.iter()
		.find(|m| m.name == name || m.aliases.contains(&name))
}

/// Returns all registered motions.
pub fn all() -> impl Iterator<Item = &'static MotionDef> {
	MOTIONS.iter()
}
