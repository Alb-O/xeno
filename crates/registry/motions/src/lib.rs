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
pub use xeno_registry_core::{Key, RegistryMetadata, RegistrySource, impl_registry_metadata};

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

/// Typed handle to a motion definition.
pub type MotionKey = Key<MotionDef>;

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
pub fn find(name: &str) -> Option<MotionKey> {
	MOTIONS
		.iter()
		.find(|m| m.name == name || m.aliases.contains(&name))
		.map(MotionKey::new)
}

/// Returns all registered motions.
pub fn all() -> impl Iterator<Item = &'static MotionDef> {
	MOTIONS.iter()
}
