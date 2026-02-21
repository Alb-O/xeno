//! Editor type definitions.
//!
//! Grouped structs for editor state management:
//! * [`crate::types::FrameState`] - Per-frame runtime state (hot fields)
//! * [`crate::runtime::work_queue::RuntimeWorkQueue`] - Deferred runtime work queue primitive for runtime convergence
//! * [`crate::types::Viewport`] - Terminal dimensions
//! * [`crate::types::Workspace`] - Session state (registers, jumps, macros)
//! * [`crate::types::Config`] - Editor configuration (theme, languages, options)
//! * [`crate::types::UndoManager`] - Editor-level undo/redo management
//! * [`crate::types::ApplyEditPolicy`] - Policy for edit transaction behavior
//! * [`crate::types::Invocation`] - Unified action/command dispatch
//! * [`crate::types::InvocationPolicy`] - Readonly enforcement policy

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
pub use frame::FrameState;
pub use history::{EditorUndoGroup, ViewSnapshot};
pub(crate) use invocation::adapters::{PipelineDisposition, PipelineLogContext, classify_for_nu_pipeline, log_pipeline_non_ok, to_command_outcome_for_nu_run};
pub use invocation::{Invocation, InvocationOutcome, InvocationPolicy, InvocationStatus, InvocationTarget};
pub use undo_manager::{UndoHost, UndoManager};
pub use viewport::Viewport;
pub use workspace::{JumpLocation, Workspace, Yank};
