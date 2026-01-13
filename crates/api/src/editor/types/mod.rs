//! Editor type definitions.
//!
//! Grouped structs for editor state management:
//! - [`FrameState`] - Per-frame runtime state (hot fields)
//! - [`Viewport`] - Terminal dimensions
//! - [`Workspace`] - Session state (registers, jumps, macros)
//! - [`Config`] - Editor configuration (theme, languages, options)

mod completion;
mod config;
mod frame;
mod history;
#[cfg(feature = "lsp")]
mod lsp_menu;
mod viewport;
mod workspace;

pub use completion::CompletionState;
pub use config::Config;
pub use frame::FrameState;
pub use history::{EditorUndoEntry, HistoryEntry};
#[cfg(feature = "lsp")]
pub use lsp_menu::{LspMenuKind, LspMenuState};
pub use viewport::Viewport;
pub use workspace::{JumpList, JumpLocation, MacroState, Registers, Workspace};
