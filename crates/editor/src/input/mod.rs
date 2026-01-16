//! Input handling: key events, modes, and pending actions.
//!
//! This module provides:
//! - [`InputHandler`] - State machine for keymap resolution and mode management
//! - [`KeyResult`] - Results from key processing (actions, mode changes, etc.)
//! - Editor integration for key and mouse event handling

pub mod handler;
pub mod insert;
mod key_handling;
mod mouse_handling;
/// Pending input state for motions, counts, and registers.
pub mod pending;
#[cfg(test)]
mod tests;
/// Input result types.
pub mod types;

pub use handler::InputHandler;
pub use types::KeyResult;
pub use xeno_primitives::Mode;
