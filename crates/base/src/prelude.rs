//! Prelude for evildoer-base.
//!
//! Re-exports the most commonly used types.

pub use ropey::{Rope, RopeSlice};

pub use crate::key::{Key, KeyCode, Modifiers, SpecialKey};
pub use crate::range::{CharIdx, Range};
pub use crate::selection::Selection;
pub use crate::transaction::{Change, Transaction};
