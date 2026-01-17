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
//! ├── buffers: HashMap<BufferId, Buffer>      // Text editing
//! ├── layout: Layout                          // Split arrangement
//! └── focused_buffer: BufferId                // Current focus
//! ```
//!
//! Views can be split horizontally or vertically, with each split containing
//! a text buffer.

pub mod buffer;
/// Buffer storage and management.
pub mod buffer_manager;
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
/// Runtime metrics for observability.
pub mod metrics;
/// Info popups for documentation and contextual help.
pub mod info_popup;
/// Input handling: key events, modes, and pending actions.
pub mod input;
/// Split layout management.
pub mod layout;
#[cfg(feature = "lsp")]
pub mod lsp;
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
/// Separator drag and hover state.
pub mod separator;
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
/// Window management and floating UI.
pub mod window;

pub use buffer::{Buffer, BufferId, HistoryResult};
pub use completion::{
	CompletionContext, CompletionItem, CompletionKind, CompletionSource, CompletionState,
};
pub use editor_ctx::{EditorCapabilities, EditorContext, EditorOps, HandleOutcome, apply_effects};
pub use impls::Editor;
pub use movement::WordType;
pub use terminal_config::{TerminalConfig, TerminalSequence};
pub use theme_source::ThemeSource;
pub use ui::UiManager;
pub use xeno_registry::themes::{
	ColorPair, ModeColors, PopupColors, SemanticColors, THEMES, Theme, ThemeColors, UiColors,
	blend_colors, get_theme, suggest_theme,
};
