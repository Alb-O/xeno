#![cfg_attr(test, allow(unused_crate_dependencies))]
//! Core editor engine: buffers, layout, overlays, and render plan generation.
//!
//! # Public API surface
//!
//! * [`Editor`] — workspace state, input handling, and plan generation.
//! * [`Buffer`], [`ViewId`] — text buffers with undo, syntax, and selections.
//! * [`render_api`] — the explicit frontend seam. All render/layout plan types
//!   that frontends consume are re-exported here.
//! * Message types ([`EditorMsg`], [`IoMsg`], [`LspMsg`], etc.) for async coordination.
//! * [`EditorContext`] / [`EditorOps`] — capability surface for commands and effects.
//! * Theme re-exports from `xeno_registry`.
//!
//! # Seam contract
//!
//! Frontends must only import from `xeno_editor::render_api` for anything
//! render/layout related. Internal modules (`completion`, `overlay`, `ui`,
//! `window`, `geometry`, `render`, `info_popup`, `snippet`) are `pub(crate)`.
//!
//! Core owns all render plan assembly — frontends receive opaque plan structs
//! with getter-only access and perform no policy decisions.

/// Theme bootstrap cache for instant first-frame rendering.
pub mod bootstrap;
pub mod buffer;
pub mod capabilities;
/// Command queue for deferred execution.
pub mod command_queue;
/// Editor-direct commands that need full [`Editor`] access.
pub mod commands;
/// Completion types and sources for command palette.
pub(crate) mod completion;
#[cfg(test)]
mod convergence;
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
pub(crate) mod geometry;
mod impls;
/// Info popups for documentation and contextual help.
pub(crate) mod info_popup;
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
pub(crate) mod overlay;
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
#[cfg(test)]
mod seam_contract;
/// Separator drag and hover state.
pub mod separator;
/// Snippet parsing and rendering primitives.
pub(crate) mod snippet;
/// Style utilities and conversions.
pub mod styles;
/// Background syntax loading manager.
pub mod syntax_manager;
/// Terminal capability configuration.
pub mod terminal_config;
pub mod test_events;
/// Editor type definitions.
pub mod types;
/// UI management: focus tracking.
pub(crate) mod ui;
/// View storage and management.
pub mod view_manager;
/// Window management primitives.
pub(crate) mod window;

pub use buffer::{Buffer, HistoryResult, ViewId};
pub(crate) use completion::CompletionState;
pub use editor_ctx::{EditorCapabilities, EditorContext, EditorOps, HandleOutcome, apply_effects};
pub use impls::{Editor, FocusReason, FocusTarget, FrontendFramePlan, PanelId};
#[cfg(feature = "lsp")]
pub use lsp::LspDiagnosticsEvent;
#[cfg(feature = "lsp")]
pub use lsp::api::LanguageServerConfig;
#[cfg(feature = "lsp")]
pub use lsp::smoke::run_lsp_smoke;
pub use msg::{Dirty, EditorMsg, IoMsg, LspMsg, MsgSender, ThemeMsg};
pub use notifications::{NotificationRenderAutoDismiss, NotificationRenderItem, NotificationRenderLevel};
pub use terminal_config::{TerminalConfig, TerminalSequence};
pub use xeno_registry::themes::{ColorPair, ModeColors, PopupColors, SemanticColors, THEMES, Theme, ThemeColors, UiColors, blend_colors, suggest_theme};
