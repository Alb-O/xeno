//! Overlay system for modal interactions and passive UI layers.
//! Anchor ID: XENO_ANCHOR_OVERLAY_SESSION
//!
//! # Purpose
//!
//! * Owns: focus-stealing modal interactions ([`crate::overlay::OverlayManager`]), passive contextual UI layers ([`crate::overlay::OverlayLayers`]), and shared type-erased state ([`crate::overlay::OverlayStore`]).
//! * Does not own: scene-layer rendering execution (owned by the UI compositor), LSP request logic.
//! * Source of truth: [`crate::overlay::OverlaySystem`].
//!
//! # Mental model
//!
//! * Terms: Session (active modal interaction), Controller (behavior logic), Context (capability surface), Layer (passive UI), Spec (declarative UI layout), Capture (pre-preview state snapshot).
//! * Lifecycle in one sentence: A controller defines a UI spec, a host allocates resources for a session, and the system restores captured state on close via a capability-limited context.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`crate::overlay::spec::OverlayUiSpec`] | Declarative UI configuration | Static geometry resolve | Controller (`ui_spec`) |
//! | [`crate::overlay::session::OverlaySession`] | Active session resources | Must be torn down | `OverlayHost::setup_session` |
//! | [`crate::overlay::session::PreviewCapture`] | Versioned state snapshot | Version-aware restore | `OverlaySession::capture_view` |
//! | [`crate::overlay::LayerEvent`] | Payloaded UI events | Broadcast to all layers | `Editor::notify_overlay_event` |
//! | [`crate::overlay::OverlayContext`] | Capability interface for overlays | Must be used instead of direct editor access | `OverlayManager::{open,commit,close}` |
//!
//! # Invariants
//!
//! * Must gate state restoration on captured buffer version matching.
//! * Must allow only one active modal session at a time.
//! * Must clamp resolved overlay areas to screen bounds.
//! * Must clear LSP UI when a modal overlay opens.
//! * Must route non-overlay module access through `OverlaySystem` accessors.
//!
//! # Data flow
//!
//! 1. Trigger: Editor calls `interaction.open(controller)`.
//! 2. Allocation: [`crate::overlay::host::OverlayHost`] resolves spec, creates scratch buffers/panes, and focuses input.
//! 3. Events: Editor emits [`crate::overlay::LayerEvent`] (CursorMoved, etc.) via `notify_overlay_event`.
//! 4. Update: Input changes in `session.input` call `controller.on_input_changed` with an [`crate::overlay::OverlayContext`].
//! 5. Restoration: On cancel/blur, `session.restore_all` reverts previews (version-aware) via the context.
//! 6. Teardown: `session.teardown` removes scratch buffers and clears pane metadata.
//!
//! # Lifecycle
//!
//! * Open: [`crate::overlay::OverlayManager::open`] calls `host.setup_session` then `controller.on_open`.
//! * Update: [`crate::overlay::OverlayManager::on_buffer_edited`] filters for `session.input`.
//! * Commit: [`crate::overlay::OverlayManager::commit`] runs [`crate::overlay::OverlayController::on_commit`] (async), then teardown.
//! * Cancel: [`crate::overlay::OverlayManager::close`] runs `session.restore_all`, then teardown.
//! * Teardown: [`crate::overlay::session::OverlaySession::teardown`] (idempotent resource cleanup).
//!
//! # Concurrency & ordering
//!
//! * Single-threaded UI: Most overlay operations run on the main UI thread.
//! * Async commit: [`crate::overlay::OverlayController::on_commit`] returns a future, allowing async operations (LSP rename) before cleanup.
//!
//! # Failure modes & recovery
//!
//! * Missing anchor: [`crate::overlay::spec::RectPolicy::Below`] returns `None` if the target role is missing; host skips that window.
//! * Stale restore: `restore_all` skips buffers with version mismatches to protect user edits.
//! * Focus loss: `CloseReason::Blur` triggers automatic cancellation if `dismiss_on_blur` is set in spec.
//!
//! # Recipes
//!
//! ## Add a new modal interaction
//!
//! 1. Create a struct implementing [`crate::overlay::OverlayController`].
//! 2. Implement `ui_spec` with [`crate::overlay::spec::RectPolicy`].
//! 3. Wire entry point in `impls::interaction`.
//! 4. Use `session.preview_select` for safe buffer previews.
//!
use std::collections::HashMap;

use xeno_primitives::range::{CharIdx, Range};
use xeno_primitives::{Mode, Selection};

use super::WindowRole;
use crate::buffer::ViewId;
use crate::geometry::Rect;
use crate::impls::FocusTarget;
use crate::overlay::OverlayContext;
use crate::window::{GutterSelector, SurfaceStyle};

mod state;

pub use state::*;
