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
pub mod capabilities;
/// Editor-direct commands that need full [`Editor`] access.
pub mod commands;
pub mod editor;
/// Info popups for documentation and contextual help.
pub mod info_popup;
#[cfg(feature = "lsp")]
pub mod lsp;
/// Type-erased UI overlay storage.
pub mod overlay;
/// Command palette for executing commands.
pub mod palette;
/// Platform-specific configuration paths.
pub mod paths;
/// Prompt overlay for one-line inputs (rename, etc).
pub mod prompt;
/// Rendering utilities for buffers, status line, and completion.
pub mod render;
/// Style utilities and conversions.
pub mod styles;
pub mod test_events;
/// UI management: focus tracking.
pub mod ui;
/// Window management and floating UI.
pub mod window;

pub use buffer::{Buffer, BufferId, HistoryResult};
pub use editor::Editor;
pub use ui::UiManager;
pub use xeno_registry::themes::{
	ColorPair, ModeColors, PopupColors, SemanticColors, THEMES, Theme, ThemeColors, UiColors,
	blend_colors, get_theme, suggest_theme,
};
