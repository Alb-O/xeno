//! Editor type definitions.
//!
//! Grouped structs for editor state management:
//! - [`FrameState`] - Per-frame runtime state (hot fields)
//! - [`Viewport`] - Terminal dimensions
//! - [`Workspace`] - Session state (registers, jumps, macros)
//! - [`Config`] - Editor configuration (theme, languages, options)

mod config;
mod frame;
mod history;
mod viewport;
mod workspace;

pub use config::Config;
pub use frame::FrameState;
pub use history::{DocumentHistoryEntry, EditorUndoGroup, ViewSnapshot};
pub use viewport::Viewport;
pub use workspace::{JumpList, JumpLocation, MacroState, Registers, Workspace};
pub use xeno_primitives::range::CharIdx;
