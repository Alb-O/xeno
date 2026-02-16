//! Frontend-agnostic runtime event loop and maintenance scheduler.
//! Anchor ID: XENO_ANCHOR_RUNTIME_LOOP
//!
//! # Purpose
//!
//! * Defines the runtime event contract consumed by frontends (`RuntimeEvent`).
//! * Owns `Editor::{on_event,pump}` ordering for UI ticks, background drains, message handling, and quit propagation.
//! * Produces loop directives (`LoopDirective`) that frontends use for polling cadence and redraw behavior.
//!
//! # Mental model
//!
//! * `on_event` applies one input/resize/focus event then always executes one `pump`.
//! * `pump` is the canonical maintenance phase; all async completion and deferred side-effects converge here.
//! * Frontends are thin adapters that feed events and obey returned loop directives.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`RuntimeEvent`] | Frontend event payload | Must be translated into editor handlers before `pump` | frontend adapters |
//! | [`LoopDirective`] | Frontend control output | Must reflect redraw/quit state after full `pump` | `Editor::pump` |
//! | [`CursorStyle`] | Editor cursor intent | Must remain mode-consistent unless UI explicitly overrides | `Editor::derive_cursor_style` |
//! | [`crate::scheduler::DrainBudget`] | Completion budget for scheduler drain | Must switch to fast budget in insert mode | `Editor::pump` |
//!
//! # Invariants
//!
//! * `on_event` must execute exactly one maintenance `pump` after applying each event.
//! * `pump` must kick queued Nu hook evaluation before draining scheduler completions.
//! * Pending overlay commit must be applied during `pump`, not during key handling.
//! * Runtime must return immediate quit directive when drained Nu hook invocations request quit.
//! * Cursor style must default to insert beam vs non-insert block when UI has no override.
//!
//! # Data flow
//!
//! 1. Frontend submits one `RuntimeEvent`.
//! 2. `on_event` routes to key/mouse/paste/resize/focus handlers.
//! 3. `pump` runs subsystem maintenance (UI tick, filesystem pump, hook kick, work drain, message drain).
//! 4. Deferred invocations and workspace edits are applied.
//! 5. Runtime emits `LoopDirective` for frontend scheduling and rendering.
//!
//! # Lifecycle
//!
//! * Startup: frontends create editor instance and begin event/pump loop.
//! * Running: repeated `on_event` and occasional direct `pump` calls drive state progression.
//! * Shutdown: `LoopDirective::should_quit` ends frontend loop.
//!
//! # Concurrency & ordering
//!
//! * Event handling and `pump` run on the editor thread.
//! * Work scheduler completions are drained under explicit budgets to preserve interactivity.
//! * Nu hook eval is kicked before draining so completions can surface quickly in subsequent cycles.
//! * Overlay commit is serialized through `pending_overlay_commit` flag and applied in `pump`.
//!
//! # Failure modes & recovery
//!
//! * Filesystem worker lag: bounded pump budget; redraw requested when new data arrives.
//! * Nu hook failures: handled in invocation pipeline; runtime continues loop.
//! * Workspace edit apply failure: user notification emitted, runtime remains live.
//! * Scheduler backlog: tracked by scheduler metrics and drop policy, loop continues.
//!
//! # Recipes
//!
//! * Add new runtime event:
//!   1. Add variant to [`RuntimeEvent`].
//!   2. Route it in `Editor::on_event`.
//!   3. Add invariant/test proving `on_event` still implies one `pump`.
//! * Add new maintenance phase:
//!   1. Insert step in `Editor::pump` with explicit placement rationale.
//!   2. Update invariants for ordering constraints.
//!   3. Add tests for redraw/quit behavior impact.

mod core;

pub use core::{CursorStyle, LoopDirective, RuntimeEvent};

#[cfg(test)]
use crate::Editor;

pub(crate) mod recorder;

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
