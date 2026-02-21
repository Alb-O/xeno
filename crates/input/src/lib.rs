//! Input handling and movement primitives for Xeno.
//!
//! This crate owns the modal input state machine ([`InputHandler`]) and pure
//! cursor/selection movement functions. It intentionally excludes editor
//! integration glue (key dispatch to editor commands, mouse handling on editor
//! windows).

mod input;
/// Pure cursor/selection movement functions.
pub mod movement;

pub use input::{InputHandler, KeyResult, types::KeyDispatch};
