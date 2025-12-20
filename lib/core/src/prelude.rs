//! Prelude for Tome core.
//!
//! Re-exports the most commonly used types for extensions.

pub use ropey::{Rope, RopeSlice};

pub use crate::ext::{
	CommandContext, CommandDef, CommandError, CommandOutcome, CommandResult, EditorOps,
};
pub use crate::input::Mode;
pub use crate::key::{Key, KeyCode, Modifiers, SpecialKey};
pub use crate::range::{CharIdx, Range};
pub use crate::selection::Selection;
pub use crate::transaction::{Change, Transaction};
