//! Modal input state machine.
//!
//! * [`InputHandler`] — keymap resolution, mode tracking, count/register accumulation
//! * [`KeyResult`] — action dispatch, mode changes, character insertion, mouse events

pub mod handler;
pub mod insert;
pub mod pending;
#[cfg(test)]
mod tests;
pub mod types;

pub use handler::InputHandler;
pub use types::KeyResult;
