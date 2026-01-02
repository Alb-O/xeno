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
//! - Standard library implementations (basic, word, line, document)

use evildoer_base::Range;
use linkme::distributed_slice;
use ropey::RopeSlice;

/// Built-in motion implementations (char, word, line, etc.).
mod impls;
/// Macro definitions for motion registration.
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
	/// Read access to document text.
	Text,
	/// Access to cursor position.
	Cursor,
	/// Access to selection state.
	Selection,
	/// Access to editor mode (normal, insert, visual).
	Mode,
	/// Ability to display messages and notifications.
	Messaging,
	/// Ability to modify document text.
	Edit,
	/// Access to search functionality.
	Search,
	/// Access to undo/redo history.
	Undo,
	/// Access to file system operations.
	FileOps,
}

/// Command flags for motion definitions.
pub mod flags {
	/// No flags set.
	pub const NONE: u32 = 0;
}

/// Common metadata for all registry item types.
///
/// Implemented by each registry definition type to enable generic
/// operations like collision detection and diagnostics.
///
/// Use [`impl_registry_metadata!`] to implement this trait.
pub trait RegistryMetadata {
	/// Returns the unique identifier for this registry item.
	fn id(&self) -> &'static str;
	/// Returns the human-readable name for this registry item.
	fn name(&self) -> &'static str;
	/// Returns the priority for collision resolution (higher wins).
	fn priority(&self) -> i16;
	/// Returns where this registry item was defined.
	fn source(&self) -> RegistrySource;
}

/// Implements [`RegistryMetadata`] for a type with `id`, `name`, `priority`, and `source` fields.
#[macro_export]
macro_rules! impl_registry_metadata {
	($type:ty) => {
		impl $crate::RegistryMetadata for $type {
			fn id(&self) -> &'static str {
				self.id
			}
			fn name(&self) -> &'static str {
				self.name
			}
			fn priority(&self) -> i16 {
				self.priority
			}
			fn source(&self) -> $crate::RegistrySource {
				self.source
			}
		}
	};
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
	/// Unique identifier for this motion.
	pub id: &'static str,
	/// Human-readable name for lookup and display.
	pub name: &'static str,
	/// Alternative names that can be used to invoke this motion.
	pub aliases: &'static [&'static str],
	/// Brief description of what this motion does.
	pub description: &'static str,
	/// Function that implements the motion logic.
	pub handler: MotionHandler,
	/// Priority for collision resolution (higher wins).
	pub priority: i16,
	/// Where this motion was defined (builtin, crate, runtime).
	pub source: RegistrySource,
	/// Capabilities required to execute this motion.
	pub required_caps: &'static [Capability],
	/// Behavioral flags for this motion.
	pub flags: u32,
}

impl_registry_metadata!(MotionDef);

impl MotionDef {
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
