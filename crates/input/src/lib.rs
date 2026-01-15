//! Input handling: key events, modes, and pending actions.

pub mod handler;
pub mod insert;
/// Pending input state for motions, counts, and registers.
pub mod pending;
#[cfg(test)]
mod tests;
/// Input result types.
pub mod types;

pub use handler::InputHandler;
pub use types::KeyResult;
pub use xeno_primitives::Mode;
