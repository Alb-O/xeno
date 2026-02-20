//! Editor input dispatch for key and mouse interaction.
//! Anchor ID: XENO_ANCHOR_INPUT_DISPATCH
//!
//! # Purpose
//!
//! * Integrates `xeno-input` modal key state with editor subsystems.
//! * Routes key and mouse events across UI panels, overlay interactions/layers, split layouts, and document buffers.
//! * Preserves deterministic ordering so modal overlays and focused panels can intercept input before base editing.
//! * Applies runtime frontend events directly on the editor thread without in-thread protocol envelopes.
//!
//! # Mental model
//!
//! * Input handling is a cascade:
//!   1. UI global/focused panel handlers.
//!   2. Active modal overlay interaction and passive overlay layers.
//!   3. LSP/snippet-specialized handlers.
//!   4. Base keymap dispatch through `xeno-input`.
//! * Mouse handling is staged:
//!   1. Build route context (drag state, overlay hit, separator hit, view hit).
//!   2. Select a single route decision (active drag, overlay, separator/view document path).
//!   3. Apply side effects for that route (focus, selection, resize, redraw).
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`xeno_primitives::Key`] | Keyboard input event | Must pass through interception cascade before base dispatch | `Editor::handle_key` |
//! | [`xeno_primitives::MouseEvent`] | Mouse input event | Must resolve hit region before applying selection/drag behavior | `Editor::handle_mouse` |
//! | [`xeno_input::input::KeyResult`] | Modal state-machine result | Must map to invocation/edit/mode transitions exactly once | `handle_key_active` |
//! | [`crate::runtime::RuntimeEvent`] | Runtime frontend event payload | Must map to one deterministic direct input application path | `Editor::apply_runtime_event_input` |
//! | [`crate::overlay::OverlaySystem`] | Modal + passive overlay state | Overlay handlers must run before base editing paths | key/mouse handling modules |
//! | [`crate::layout::manager::LayoutManager`] | Split/layout interaction state | Separator drags and view-local selection must use layout geometry | mouse handling module |
//!
//! # Invariants
//!
//! * Must allow active overlay interaction/layers to consume input before base keymap dispatch.
//! * Must defer overlay commit execution via runtime work queue drain phases.
//! * Must route keymap-produced action/command invocations through `Editor::run_invocation`.
//! * Must apply runtime frontend events deterministically through direct editor-thread calls.
//! * Deferred input consequences must cross runtime facade ports during pump drain phases.
//! * Must confine drag-selection updates to the origin view during active text-selection drags.
//! * Must cancel or ignore stale separator drag paths after structural layout changes.
//! * Mouse/panel focus transitions must synchronize editor focus after UI handling.
//!
//! # Data flow
//!
//! 1. Runtime receives key/mouse event and forwards to this subsystem.
//! 2. Input cascade determines interception target (UI, overlay, base view).
//! 3. Base dispatch returns `KeyResult` that maps to canonical invocations or local edit/mode behavior.
//! 4. Canonical invocations flow through invocation preflight/execution; local effects are applied directly.
//! 5. Runtime drain phases apply deferred commit/drain consequences through runtime facade ports.
//!
//! # Lifecycle
//!
//! * Initialization: editor starts with base keymap and no active drag/overlay input captures.
//! * Active loop: each input event flows through deterministic cascade.
//! * Structural changes: layout/overlay revisions invalidate stale drag/select references.
//! * Shutdown: no persistent worker state; input lifecycle ends with editor runtime.
//!
//! # Concurrency & ordering
//!
//! * Input handling is synchronous on the editor thread.
//! * Ordering is semantic: UI/overlay interception must precede base keymap dispatch.
//! * Deferred overlay commits are serialized by runtime drain phases.
//!
//! # Failure modes & recovery
//!
//! * Unknown/unsupported key results are treated as consumed/unhandled safely.
//! * Click outside modal overlay triggers dismiss/blur handling.
//! * Stale drag state is dropped to avoid resizing/selecting wrong targets.
//!
//! # Recipes
//!
//! * Add a new key interception layer:
//!   1. Insert layer before base keymap dispatch in `handle_key_active`.
//!   2. Return early on consume.
//!   3. Add invariant proof for precedence.
//! * Add a new mouse interaction mode:
//!   1. Extend route context fields in `mouse_handling::context`.
//!   2. Add route selection logic in `mouse_handling::routing`.
//!   3. Add side-effect application in `mouse_handling::effects` and invariant tests.

mod key_handling;
mod mouse_handling;

use xeno_primitives::KeyCode;

use crate::Editor;
use crate::runtime::RuntimeEvent;
use crate::types::{Invocation, InvocationPolicy};

impl Editor {
	pub(crate) async fn apply_input_invocation_request(&mut self, invocation: Invocation, policy: InvocationPolicy) -> bool {
		let outcome = self.run_invocation(invocation, policy).await;
		if outcome.is_quit() {
			self.request_quit();
			return true;
		}
		false
	}

	/// Applies one runtime frontend event through direct editor-thread input handling.
	pub(crate) async fn apply_runtime_event_input(&mut self, event: RuntimeEvent) -> bool {
		match event {
			RuntimeEvent::Key(key) => {
				if self.state.ui.overlay_system.interaction().is_open() && key.code == KeyCode::Enter {
					self.enqueue_runtime_overlay_commit_work();
					self.state.core.frame.needs_redraw = true;
					self.interaction_on_buffer_edited();
					return false;
				}
				if self.handle_key(key).await {
					self.request_quit();
					return true;
				}
			}
			RuntimeEvent::Mouse(mouse) => {
				if self.handle_mouse(mouse).await {
					self.request_quit();
					return true;
				}
			}
			RuntimeEvent::Paste(content) => {
				self.handle_paste(content);
			}
			RuntimeEvent::WindowResized { cols, rows } => {
				self.handle_window_resize(cols, rows);
			}
			RuntimeEvent::FocusIn => {
				self.handle_focus_in();
				self.sync_focus_from_ui();
			}
			RuntimeEvent::FocusOut => {
				self.handle_focus_out();
				self.sync_focus_from_ui();
			}
		}

		false
	}
}

#[cfg(test)]
mod invariants;
