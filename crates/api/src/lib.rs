//! Editor engine and terminal UI infrastructure.
//!
//! This crate provides the core editor implementation, buffer management,
//! and terminal rendering. It ties together [`tome_manifest`] (registry definitions)
//! and [`tome_stdlib`] (implementations) into a working editor.
//!
//! # Main Types
//!
//! - [`Editor`] - The main editor/workspace containing buffers and state
//! - [`Buffer`] - A text buffer with undo history, syntax highlighting, and selections
//! - [`TerminalBuffer`] - Embedded terminal emulator for shell integration
//! - [`UiManager`] - Panel and dock management for the UI
//!
//! # Architecture
//!
//! The editor supports heterogeneous views through [`buffer::BufferView`]:
//!
//! ```text
//! Editor
//! ├── buffers: HashMap<BufferId, Buffer>      // Text editing
//! ├── terminals: HashMap<TerminalId, TerminalBuffer>  // Shell integration
//! ├── layout: Layout                          // Split arrangement
//! └── focused_view: BufferView                // Current focus
//! ```
//!
//! Views can be split horizontally or vertically, with each split containing
//! either a text buffer or a terminal.

pub mod buffer;
pub mod capabilities;
pub mod editor;
#[cfg(feature = "lsp")]
pub mod lsp;
pub mod paths;
pub mod render;
pub mod styles;
pub mod terminal;
pub mod terminal_ipc;
pub mod test_events;
pub mod ui;

pub use buffer::{Buffer, BufferId, HistoryResult};
pub use editor::Editor;
pub use terminal::TerminalBuffer;
pub use terminal_ipc::{IpcRequest, TerminalIpc, TerminalIpcEnv};
pub use tome_theme::{
	PopupColors, StatusColors, THEMES, Theme, ThemeColors, UiColors, blend_colors, get_theme,
	suggest_theme,
};
pub use ui::UiManager;
