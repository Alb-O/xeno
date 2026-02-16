//! Editor type definitions.
//!
//! Grouped structs for editor state management:
//! * [`crate::types::FrameState`] - Per-frame runtime state (hot fields)
//! * [`crate::types::DeferredWorkQueue`] - Deferred runtime work backlog for pump convergence
//! * [`crate::types::Viewport`] - Terminal dimensions
//! * [`crate::types::Workspace`] - Session state (registers, jumps, macros)
//! * [`crate::types::Config`] - Editor configuration (theme, languages, options)
//! * [`crate::types::UndoManager`] - Editor-level undo/redo management
//! * [`crate::types::ApplyEditPolicy`] - Policy for edit transaction behavior
//! * [`crate::types::Invocation`] - Unified action/command dispatch
//! * [`crate::types::InvocationPolicy`] - Capability enforcement policy

mod config;
mod edit_policy;
mod frame;
mod history;
mod invocation;
mod undo_manager;
mod viewport;
mod workspace;

pub use config::Config;
pub use edit_policy::ApplyEditPolicy;
pub use frame::{DeferredWorkItem, DeferredWorkQueue, FrameState};
pub use history::{DocumentHistoryEntry, EditorUndoGroup, ViewSnapshot};
pub use invocation::{Invocation, InvocationPolicy, InvocationResult};
pub use undo_manager::{PreparedEdit, UndoHost, UndoManager};
pub use viewport::Viewport;
pub use workspace::{JumpList, JumpLocation, MacroState, Registers, Workspace, Yank};
pub use xeno_primitives::range::CharIdx;
