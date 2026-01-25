//! Editor engine and terminal UI infrastructure.
//!
//! This crate provides the core editor implementation, buffer management,
//! and terminal rendering.
//!
//! # Main Types
//!
//! - [`Editor`] - The main editor/workspace containing buffers and state
//! - [`Buffer`] - A text buffer with undo history, syntax highlighting, and selections
//! - [`UiManager`] - UI management for the editor
//!
//! # Architecture
//!
//! The editor uses a split-based layout for text buffers:
//!
//! ```text
//! Editor
//! ├── buffers: HashMap<ViewId, Buffer>        // Text editing
//! ├── layout: Layout                          // Split arrangement
//! └── focused_buffer: ViewId                  // Current focus
//! ```
//!
//! Views can be split horizontally or vertically, with each split containing
//! a text buffer.

/// Theme bootstrap cache for instant first-frame rendering.
pub mod bootstrap;
pub mod buffer;
pub mod capabilities;
/// Command queue for deferred execution.
pub mod command_queue;
/// Editor-direct commands that need full [`Editor`] access.
pub mod commands;
/// Completion types and sources for command palette.
pub mod completion;
/// Editor context and effect handling.
pub mod editor_ctx;
/// Extension container and style overlays.
pub mod extensions;
/// Async hook execution runtime.
pub mod hook_runtime;
pub mod impls;
/// Async message bus for background task hydration.
pub mod msg;
/// Info popups for documentation and contextual help.
pub mod info_popup;
/// Input handling: key events, modes, and pending actions.
pub mod input;
/// Split layout management.
pub mod layout;
#[cfg(feature = "lsp")]
pub mod lsp;
#[path = "lsp/system.rs"]
mod lsp_system;
/// Runtime metrics for observability.
pub mod metrics;
/// Cursor movement functions.
pub mod movement;
/// Type-erased UI overlay storage.
pub mod overlay;
/// Command palette for executing commands.
pub mod palette;
/// Platform-specific configuration paths.
pub mod paths;
/// Prompt overlay for one-line inputs.
pub mod prompt;
/// Rendering utilities for buffers, status line, and completion.
pub mod render;
/// Unified async work scheduler.
pub mod scheduler;
/// Separator drag and hover state.
pub mod separator;
/// Background syntax loading manager.
pub mod syntax_manager;
/// Style utilities and conversions.
pub mod styles;
/// Terminal capability configuration.
pub mod terminal_config;
pub mod test_events;
/// Theme completion source.
pub mod theme_source;
/// Editor type definitions.
pub mod types;
/// UI management: focus tracking.
pub mod ui;
/// View storage and management.
pub mod view_manager;
/// Window management and floating UI.
pub mod window;

pub use buffer::{Buffer, HistoryResult, ViewId};
pub use completion::{
	CompletionContext, CompletionItem, CompletionKind, CompletionSource, CompletionState,
};
pub use editor_ctx::{EditorCapabilities, EditorContext, EditorOps, HandleOutcome, apply_effects};
pub use impls::Editor;
pub use lsp_system::LspSystem;
pub use movement::WordType;
pub use terminal_config::{TerminalConfig, TerminalSequence};
pub use theme_source::ThemeSource;
pub use msg::{Dirty, EditorMsg, IoMsg, LspMsg, MsgSender, ThemeMsg};
pub use ui::UiManager;
pub use xeno_registry::themes::{
	ColorPair, ModeColors, PopupColors, SemanticColors, THEMES, Theme, ThemeColors, UiColors,
	blend_colors, get_theme, suggest_theme,
};

#[cfg(test)]
mod smoke_tests;
