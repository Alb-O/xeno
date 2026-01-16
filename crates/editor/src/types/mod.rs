//! Editor type definitions.
//!
//! Grouped structs for editor state management:
//! - [`FrameState`] - Per-frame runtime state (hot fields)
//! - [`Viewport`] - Terminal dimensions
//! - [`Workspace`] - Session state (registers, jumps, macros)
//! - [`Config`] - Editor configuration (theme, languages, options)
//! - [`UndoManager`] - Editor-level undo/redo management
//! - [`ApplyEditPolicy`] - Policy for edit transaction behavior

mod config;
mod edit_policy;
mod frame;
mod history;
mod undo_manager;
mod viewport;
mod workspace;

pub use config::Config;
pub use edit_policy::ApplyEditPolicy;
pub use frame::FrameState;
pub use history::{DocumentHistoryEntry, EditorUndoGroup, ViewSnapshot};
pub use undo_manager::{PreparedEdit, UndoHost, UndoManager};
pub use viewport::Viewport;
pub use workspace::{JumpList, JumpLocation, MacroState, Registers, Workspace};
pub use xeno_primitives::range::CharIdx;
