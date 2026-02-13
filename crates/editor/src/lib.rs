#![cfg_attr(test, allow(unused_crate_dependencies))]
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
#[cfg(test)]
mod convergence;
#[cfg(test)]
mod seam_contract;
/// Headless core model (documents, undo).
pub mod core;
/// Editor context and effect handling.
pub mod editor_ctx;
/// Unified side-effect routing and sink.
pub mod effects;
/// Execution gate for task ordering.
pub mod execution_gate;
/// Filesystem indexing and picker backend services.
pub(crate) mod filesystem;
/// Shared geometry aliases for core/front-end seams.
pub mod geometry;
/// Async hook execution runtime.
pub mod hook_runtime;
mod impls;
/// Info popups for documentation and contextual help.
pub mod info_popup;
/// Editor key/mouse dispatch (input state machine lives in `xeno-input`).
mod input;
/// Split layout management.
pub mod layout;
mod lsp;
/// Runtime metrics for observability.
pub mod metrics;
/// Async message bus for background task hydration.
pub mod msg;
/// User notification queue internals.
mod notifications;
/// Nu runtime for user macro scripts.
pub mod nu;
/// Type-erased UI overlay storage.
pub mod overlay;
pub(crate) mod paste;
/// Platform-specific configuration paths.
pub mod paths;
/// Internal rendering utilities for buffers, status line, and completion.
mod render;
/// Frontend-facing render boundary exports.
pub mod render_api;
/// Runtime policy and directives.
pub mod runtime;
/// Unified async work scheduler.
pub mod scheduler;
/// Separator drag and hover state.
pub mod separator;
/// Snippet parsing and rendering primitives.
pub mod snippet;
/// Style utilities and conversions.
pub mod styles;
/// Background syntax loading manager.
pub mod syntax_manager;
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
/// Window management primitives.
pub mod window;

pub use buffer::{Buffer, HistoryResult, ViewId};
pub use completion::{CompletionContext, CompletionItem, CompletionKind, CompletionSource, CompletionState};
pub use editor_ctx::{EditorCapabilities, EditorContext, EditorOps, HandleOutcome, apply_effects};
pub use impls::{Editor, FocusReason, FocusTarget, PanelId};
#[cfg(feature = "lsp")]
pub use lsp::LspDiagnosticsEvent;
#[cfg(feature = "lsp")]
pub use lsp::api::LanguageServerConfig;
#[cfg(feature = "lsp")]
pub use lsp::smoke::run_lsp_smoke;
pub use msg::{Dirty, EditorMsg, IoMsg, LspMsg, MsgSender, ThemeMsg};
pub use notifications::{NotificationRenderAutoDismiss, NotificationRenderItem, NotificationRenderLevel};
pub use terminal_config::{TerminalConfig, TerminalSequence};
pub use theme_source::ThemeSource;
pub use ui::UiManager;
pub use xeno_registry::themes::{ColorPair, ModeColors, PopupColors, SemanticColors, THEMES, Theme, ThemeColors, UiColors, blend_colors, suggest_theme};
