//! Editor input dispatch for key and mouse interaction.
//! Anchor ID: XENO_ANCHOR_INPUT_DISPATCH
//!
//! # Purpose
//!
//! * Integrates `xeno-input` modal key state with editor subsystems.
//! * Routes key and mouse events across UI panels, overlay interactions/layers, split layouts, and document buffers.
//! * Preserves deterministic ordering so modal overlays and focused panels can intercept input before base editing.
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
//! | [`crate::overlay::OverlaySystem`] | Modal + passive overlay state | Overlay handlers must run before base editing paths | key/mouse handling modules |
//! | [`crate::layout::manager::LayoutManager`] | Split/layout interaction state | Separator drags and view-local selection must use layout geometry | mouse handling module |
//!
//! # Invariants
//!
//! * Must allow active overlay interaction/layers to consume input before base keymap dispatch.
//! * Must defer overlay commit execution to runtime `pump` via pending-commit flag.
//! * Must route keymap-produced action/command invocations through `Editor::run_invocation`.
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
//! 5. Runtime `pump` applies deferred commit/drain consequences.
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
//! * Deferred overlay commits are serialized by runtime `pump`.
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

#[cfg(test)]
mod invariants;
