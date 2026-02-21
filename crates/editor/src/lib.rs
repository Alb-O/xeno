#![cfg_attr(test, allow(unused_crate_dependencies))]
//! Core editor engine: buffers, layout, overlays, and render plan generation.
//!
//! # Public API surface
//!
//! * [`Editor`] — workspace state, input handling, and plan generation.
//! * [`Buffer`], [`ViewId`] — text buffers with undo, syntax, and selections.
//! * Render/layout plan types from the internal `render_api` module are
//!   re-exported directly at the crate root for frontend consumption.
//! * Message types ([`EditorMsg`], [`IoMsg`], [`LspMsg`], etc.) for async coordination.
//! * [`EditorContext`] / [`EditorOps`] — capability surface for commands and effects.
//! * Theme re-exports from `xeno_registry`.
//!
//! # Seam contract
//!
//! Frontends must only import from the `xeno_editor` crate root for anything
//! render/layout related. All render plan types are re-exported at the root.
//! Internal modules (`completion`, `overlay`, `ui`, `window`, `geometry`,
//! `render`, `info_popup`, `snippet`) are `pub(crate)`.
//!
//! Core owns all render plan assembly — frontends receive opaque plan structs
//! with getter-only access and perform no policy decisions.

/// Theme bootstrap cache for instant first-frame rendering.
mod bootstrap;
mod buffer;
mod buffer_identity;
mod capabilities;
/// Editor-direct commands that need full [`Editor`] access.
mod commands;
/// Completion types and sources for command palette.
pub(crate) mod completion;
#[cfg(test)]
mod convergence;
/// Headless core model (documents, undo).
mod core;
/// Editor context and effect handling.
mod editor_ctx;
/// Unified side-effect routing and sink.
mod effects;
/// Execution gate for task ordering.
mod execution_gate;
/// Filesystem indexing and picker backend services.
pub(crate) mod filesystem;
/// Shared geometry aliases for core/front-end seams.
pub(crate) mod geometry;
mod impls;
/// Info popups for documentation and contextual help.
pub(crate) mod info_popup;
/// Editor key/mouse dispatch (input state machine lives in `xeno-input`).
mod input;
/// Atomic file writing utilities.
pub(crate) mod io;
/// Split layout management.
mod layout;
mod lsp;
/// Runtime metrics for observability.
mod metrics;
/// Async message bus for background task hydration.
mod msg;
/// User notification queue internals.
mod notifications;
/// Nu runtime for user macro scripts.
mod nu;
/// Type-erased UI overlay storage.
pub(crate) mod overlay;
pub(crate) mod paste;
/// Platform-specific configuration paths.
mod paths;
/// Internal rendering utilities for buffers, status line, and completion.
mod render;
/// Frontend-facing render boundary exports.
mod render_api;
/// Runtime policy and directives.
mod runtime;
/// Unified async work scheduler.
mod scheduler;
#[cfg(test)]
mod seam_contract;
/// Separator drag and hover state.
mod separator;
/// Snippet parsing and rendering primitives.
pub(crate) mod snippet;
/// Style utilities and conversions.
mod styles;
/// Terminal capability configuration.
mod terminal_config;
mod test_events;
/// Editor type definitions.
mod types;
/// UI management: focus tracking.
pub(crate) mod ui;
/// View storage and management.
mod view_manager;
/// Window management primitives.
pub(crate) mod window;

// Root facade re-exports for external consumers.
pub use bootstrap::init as bootstrap_init;
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
pub use paths::get_data_dir;
pub use render_api::{
	CompletionKind, CompletionRenderItem, CompletionRenderPlan, DocumentViewPlan, FilePresentationRender, InfoPopupId, InfoPopupRenderAnchor,
	InfoPopupRenderTarget, OverlayControllerKind, OverlayPaneRenderTarget, PanelRenderTarget, Rect, RenderLine, SeparatorJunctionTarget, SeparatorRenderTarget,
	SeparatorState, SnippetChoiceRenderItem, SnippetChoiceRenderPlan, SplitDirection, StatuslineRenderSegment, StatuslineRenderStyle, SurfaceStyle,
	UTILITY_PANEL_ID, WindowRole,
};
pub use runtime::{CursorStyle, DrainPolicy, LoopDirectiveV2, RuntimeEvent};
pub use styles::cli_styles;
pub use terminal_config::{TerminalConfig, TerminalSequence};
pub use test_events::SeparatorAnimationEvent;
pub use xeno_registry::themes::{ColorPair, ModeColors, PopupColors, SemanticColors, THEMES, Theme, ThemeColors, UiColors, blend_colors, suggest_theme};
