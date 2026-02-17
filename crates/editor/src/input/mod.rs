//! Editor input dispatch for key and mouse interaction.
//! Anchor ID: XENO_ANCHOR_INPUT_DISPATCH
//!
//! # Purpose
//!
//! * Integrates `xeno-input` modal key state with editor subsystems.
//! * Routes key and mouse events across UI panels, overlay interactions/layers, split layouts, and document buffers.
//! * Preserves deterministic ordering so modal overlays and focused panels can intercept input before base editing.
//! * Produces typed input envelopes (`InputDispatchCmd`/`InputDispatchEvt`) for runtime-owned event coordination.
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
//! | [`protocol::InputDispatchCmd`] | Runtime-owned input command envelope | Must be transformed into typed dispatch events before execution | `Editor::dispatch_input_cmd` |
//! | [`protocol::InputDispatchEvt`] | Input dispatch event envelope | Must be consumed in-order by runtime coordinator | `Editor::apply_input_dispatch_evt` |
//! | [`crate::overlay::OverlaySystem`] | Modal + passive overlay state | Overlay handlers must run before base editing paths | key/mouse handling modules |
//! | [`crate::layout::manager::LayoutManager`] | Split/layout interaction state | Separator drags and view-local selection must use layout geometry | mouse handling module |
//!
//! # Invariants
//!
//! * Must allow active overlay interaction/layers to consume input before base keymap dispatch.
//! * Must defer overlay commit execution via runtime work queue drain phases.
//! * Must route keymap-produced action/command invocations through `Editor::run_invocation`.
//! * Must emit and apply `InputDispatchEvt` envelopes in-order for each `InputDispatchCmd`.
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
//! 5. Runtime drain phases apply deferred commit/drain consequences.
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
pub(crate) mod protocol;

use protocol::{InputDispatchCmd, InputDispatchEvt, InputLocalEffect, LayoutActionRequest};
use xeno_primitives::KeyCode;

use crate::Editor;
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

	/// Produces typed input events from one runtime-owned input command envelope.
	pub(crate) async fn dispatch_input_cmd(&mut self, cmd: InputDispatchCmd) -> Vec<InputDispatchEvt> {
		match cmd {
			InputDispatchCmd::Key(key) => {
				if self.state.overlay_system.interaction().is_open() && key.code == KeyCode::Enter {
					vec![
						InputDispatchEvt::OverlayCommitDeferred,
						InputDispatchEvt::LayoutActionRequested(LayoutActionRequest::InteractionBufferEdited),
						InputDispatchEvt::Consumed,
					]
				} else {
					vec![
						InputDispatchEvt::LocalEffectRequested(InputLocalEffect::DispatchKey(key)),
						InputDispatchEvt::Consumed,
					]
				}
			}
			InputDispatchCmd::Mouse(mouse) => vec![
				InputDispatchEvt::LocalEffectRequested(InputLocalEffect::DispatchMouse(mouse)),
				InputDispatchEvt::Consumed,
			],
			InputDispatchCmd::Paste(content) => vec![
				InputDispatchEvt::LocalEffectRequested(InputLocalEffect::ApplyPaste(content)),
				InputDispatchEvt::Consumed,
			],
			InputDispatchCmd::Resize { cols, rows } => vec![
				InputDispatchEvt::LocalEffectRequested(InputLocalEffect::ApplyResize { cols, rows }),
				InputDispatchEvt::Consumed,
			],
			InputDispatchCmd::FocusIn => vec![
				InputDispatchEvt::LocalEffectRequested(InputLocalEffect::ApplyFocusIn),
				InputDispatchEvt::FocusSyncRequested,
				InputDispatchEvt::Consumed,
			],
			InputDispatchCmd::FocusOut => vec![
				InputDispatchEvt::LocalEffectRequested(InputLocalEffect::ApplyFocusOut),
				InputDispatchEvt::FocusSyncRequested,
				InputDispatchEvt::Unhandled,
			],
		}
	}

	/// Applies one typed input dispatch event emitted by [`Self::dispatch_input_cmd`].
	pub(crate) async fn apply_input_dispatch_evt(&mut self, event: InputDispatchEvt) -> bool {
		match event {
			InputDispatchEvt::InvocationRequested { invocation, policy } => {
				if self.apply_input_invocation_request(invocation, policy).await {
					return true;
				}
			}
			InputDispatchEvt::LocalEffectRequested(effect) => match effect {
				InputLocalEffect::DispatchKey(key) => {
					if self.handle_key(key).await {
						self.request_quit();
						return true;
					}
				}
				InputLocalEffect::DispatchMouse(mouse) => {
					if self.handle_mouse(mouse).await {
						self.request_quit();
						return true;
					}
				}
				InputLocalEffect::ApplyPaste(content) => {
					self.handle_paste(content);
				}
				InputLocalEffect::ApplyResize { cols, rows } => {
					self.handle_window_resize(cols, rows);
				}
				InputLocalEffect::ApplyFocusIn => {
					self.handle_focus_in();
				}
				InputLocalEffect::ApplyFocusOut => {
					self.handle_focus_out();
				}
			},
			InputDispatchEvt::OverlayCommitDeferred => {
				self.enqueue_runtime_overlay_commit_work();
				self.state.frame.needs_redraw = true;
			}
			InputDispatchEvt::LayoutActionRequested(LayoutActionRequest::InteractionBufferEdited) => {
				self.interaction_on_buffer_edited();
			}
			InputDispatchEvt::FocusSyncRequested => {
				self.sync_focus_from_ui();
			}
			InputDispatchEvt::Consumed | InputDispatchEvt::Unhandled => {}
		}

		false
	}

	/// Applies a sequence of typed input events in-order.
	pub(crate) async fn apply_input_dispatch_events(&mut self, events: Vec<InputDispatchEvt>) {
		for event in events {
			let _ = self.apply_input_dispatch_evt(event).await;
		}
	}
}

#[cfg(test)]
mod invariants;
