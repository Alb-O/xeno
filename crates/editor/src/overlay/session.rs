//! Overlay system for modal interactions and passive UI layers.
//!
//! # Purpose
//!
//! - Owns: focus-stealing modal interactions ([`crate::overlay::OverlayManager`]), passive contextual UI layers ([`crate::overlay::OverlayLayers`]), and shared type-erased state ([`crate::overlay::OverlayStore`]).
//! - Does not own: floating window rendering (owned by window subsystem), LSP request logic.
//! - Source of truth: [`crate::overlay::OverlaySystem`].
//!
//! # Mental model
//!
//! - Terms: Session (active modal interaction), Controller (behavior logic), Context (capability surface), Layer (passive UI), Spec (declarative UI layout), Capture (pre-preview state snapshot).
//! - Lifecycle in one sentence: A controller defines a UI spec, a host allocates resources for a session, and the system restores captured state on close via a capability-limited context.
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
//! - Must gate state restoration on captured buffer version matching.
//! - Must allow only one active modal session at a time.
//! - Must clamp resolved overlay areas to screen bounds.
//! - Must clear LSP UI when a modal overlay opens.
//!
//! # Data flow
//!
//! 1. Trigger: Editor calls `interaction.open(controller)`.
//! 2. Allocation: [`crate::overlay::host::OverlayHost`] resolves spec, creates scratch buffers/windows, and focuses input.
//! 3. Events: Editor emits [`crate::overlay::LayerEvent`] (CursorMoved, etc.) via `notify_overlay_event`.
//! 4. Update: Input changes in `session.input` call `controller.on_input_changed` with an [`crate::overlay::OverlayContext`].
//! 5. Restoration: On cancel/blur, `session.restore_all` reverts previews (version-aware) via the context.
//! 6. Teardown: `session.teardown` closes all windows and removes buffers.
//!
//! # Lifecycle
//!
//! - Open: [`crate::overlay::OverlayManager::open`] calls `host.setup_session` then `controller.on_open`.
//! - Update: [`crate::overlay::OverlayManager::on_buffer_edited`] filters for `session.input`.
//! - Commit: [`crate::overlay::OverlayManager::commit`] runs [`crate::overlay::OverlayController::on_commit`] (async), then teardown.
//! - Cancel: [`crate::overlay::OverlayManager::close`] runs `session.restore_all`, then teardown.
//! - Teardown: [`crate::overlay::session::OverlaySession::teardown`] (idempotent resource cleanup).
//!
//! # Concurrency & ordering
//!
//! - Single-threaded UI: Most overlay operations run on the main UI thread.
//! - Async commit: [`crate::overlay::OverlayController::on_commit`] returns a future, allowing async operations (LSP rename) before cleanup.
//!
//! # Failure modes & recovery
//!
//! - Missing anchor: [`crate::overlay::spec::RectPolicy::Below`] returns `None` if the target role is missing; host skips that window.
//! - Stale restore: `restore_all` skips buffers with version mismatches to protect user edits.
//! - Focus loss: `CloseReason::Blur` triggers automatic cancellation if `dismiss_on_blur` is set in spec.
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
use xeno_tui::layout::Rect;

use crate::buffer::ViewId;
use crate::impls::FocusTarget;
use crate::overlay::OverlayContext;
use crate::window::{FloatingStyle, GutterSelector};
use super::WindowRole;

/// Renderable pane metadata for a modal overlay session.
pub struct OverlayPane {
	pub role: WindowRole,
	pub buffer: ViewId,
	pub rect: Rect,
	pub content_rect: Rect,
	pub style: FloatingStyle,
	pub gutter: GutterSelector,
	pub dismiss_on_blur: bool,
	pub sticky: bool,
}

/// State and resources for an active modal interaction session.
///
/// An `OverlaySession` is created by [`crate::overlay::OverlayHost`] and managed by [`crate::overlay::OverlayManager`].
/// It tracks all allocated UI resources and provides mechanisms for temporary
/// state capture and restoration.
pub struct OverlaySession {
	/// List of panes rendered for this session.
	pub panes: Vec<OverlayPane>,
	/// List of scratch buffer IDs allocated for this session.
	pub buffers: Vec<ViewId>,
	/// The primary input buffer ID for the interaction.
	pub input: ViewId,

	/// The focus target to restore after the session ends.
	pub origin_focus: FocusTarget,
	/// The editor mode to restore after the session ends.
	pub origin_mode: Mode,
	/// The buffer view that was active when the session started.
	pub origin_view: ViewId,

	/// Storage for captured buffer states (cursor, selection) for restoration.
	pub capture: PreviewCapture,

	/// Current status message displayed by the overlay.
	pub status: OverlayStatus,
}

/// Storage for buffer states captured before transient changes.
#[derive(Default)]
pub struct PreviewCapture {
	/// Mapping of view ID to (version, cursor position, selection).
	pub per_view: HashMap<ViewId, CapturedViewState>,
}

#[derive(Debug, Clone)]
pub struct CapturedViewState {
	pub version: u64,
	pub cursor: CharIdx,
	pub selection: Selection,
}

/// Metadata about the current session status.
#[derive(Debug, Default, Clone)]
pub struct OverlayStatus {
	/// Optional status message and its severity kind.
	pub message: Option<(StatusKind, String)>,
}

/// Severity kind for overlay status messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
	Info,
	Warn,
	Error,
}

impl OverlaySession {
	/// Returns the current text content of the primary input buffer.
	pub fn input_text(&self, ctx: &dyn OverlayContext) -> String {
		ctx.buffer(self.input)
			.map(|b| b.with_doc(|doc| doc.content().to_string()))
			.unwrap_or_default()
	}

	/// Captures the current state of a view if it hasn't been captured yet.
	///
	/// Use this before applying preview modifications to a buffer to ensure
	/// the original state can be restored.
	pub fn capture_view(&mut self, ctx: &dyn OverlayContext, view: ViewId) {
		if self.capture.per_view.contains_key(&view) {
			return;
		}
		if let Some(buffer) = ctx.buffer(view) {
			self.capture.per_view.insert(
				view,
				CapturedViewState {
					version: buffer.version(),
					cursor: buffer.cursor,
					selection: buffer.selection.clone(),
				},
			);
		}
	}

	/// Selects a range in a view, capturing its state first if necessary.
	pub fn preview_select(&mut self, ctx: &mut dyn OverlayContext, view: ViewId, range: Range) {
		self.capture_view(ctx, view);
		if let Some(buffer) = ctx.buffer_mut(view) {
			let start = range.min();
			let end = range.max();
			let selection = Selection::single(start, end);
			buffer.set_cursor_and_selection(start, selection);
		}
	}

	/// Restores all captured view states.
	///
	/// Only restores a buffer if its version still matches the captured version,
	/// preventing user edits from being clobbered by stale preview restoration.
	///
	/// This is non-destructive; the capture map remains intact until
	/// [`Self::clear_capture`] is called.
	pub fn restore_all(&self, ctx: &mut dyn OverlayContext) {
		for (view, captured) in &self.capture.per_view {
			if let Some(buffer) = ctx.buffer_mut(*view)
				&& buffer.version() == captured.version
			{
				buffer.set_cursor_and_selection(captured.cursor, captured.selection.clone());
			}
		}
	}

	/// Destroys all captured view state.
	pub fn clear_capture(&mut self) {
		self.capture.per_view.clear();
	}

	/// Sets the session status message.
	pub fn set_status(&mut self, kind: StatusKind, msg: impl Into<String>) {
		self.status.message = Some((kind, msg.into()));
	}

	/// Clears the session status message.
	pub fn clear_status(&mut self) {
		self.status.message = None;
	}

	/// Cleans up all resources allocated for the session.
	///
	/// Removes scratch buffers.
	/// Safe to call multiple times.
	pub fn teardown(&mut self, ctx: &mut dyn OverlayContext) {
		self.panes.clear();
		for buffer_id in self.buffers.drain(..) {
			ctx.finalize_buffer_removal(buffer_id);
		}
		self.clear_capture();
	}
}
