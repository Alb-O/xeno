//! Frontend-agnostic runtime event loop and bounded-convergence pump pipeline.
//! Anchor ID: XENO_ANCHOR_RUNTIME_LOOP
//!
//! # Purpose
//!
//! * Defines the frontend/runtime event boundary (`RuntimeEvent`).
//! * Owns `Editor::{on_event,pump}` orchestration for deferred work convergence and quit propagation.
//! * Produces `LoopDirective` values that frontends use for redraw and polling cadence.
//!
//! # Mental model
//!
//! * `on_event` applies exactly one frontend event, then calls one `pump`.
//! * `pump` runs ordered maintenance phases in bounded rounds (`MAX_PUMP_ROUNDS`), trading single-call latency for deterministic caps.
//! * Each round is explicit and reported (`PumpCycleReport`) so ordering and progress policy stay testable.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`RuntimeEvent`] | Frontend event payload | Must be translated into editor handlers before `pump` | frontend adapters |
//! | [`LoopDirective`] | Frontend control output | Must reflect redraw/quit after full cycle | `Editor::pump` |
//! | [`CursorStyle`] | Editor cursor intent | Must remain mode-consistent unless UI explicitly overrides | `Editor::derive_cursor_style` |
//! | [`pump::PumpCycleReport`] | Internal round/phase progress report | Must preserve phase order and cap tracking for invariants/tests | `pump::run_pump_cycle_with_report` |
//! | [`pump::RoundWorkFlags`] | Per-round progress summary | Must drive bounded-convergence continuation policy | `pump::run_pump_cycle_with_report` |
//!
//! # Invariants
//!
//! * `on_event` must execute exactly one maintenance `pump` after applying each event.
//! * Each pump cycle must execute no more than `pump::MAX_PUMP_ROUNDS` rounds.
//! * Each round must preserve phase ordering: tick -> filesystem -> overlay commit -> messages -> workspace edits -> Nu kick -> scheduler -> commands -> hook invocations.
//! * Pump must kick queued Nu hook eval before scheduler drain in each round.
//! * Pending overlay commit must be applied during `pump`, not during key handling.
//! * Runtime must return immediate quit directive when command or hook drain requests quit.
//! * Cursor style must default to insert beam vs non-insert block when UI has no override.
//!
//! # Data flow
//!
//! 1. Frontend submits one `RuntimeEvent`.
//! 2. `on_event` routes key/mouse/paste/resize/focus handlers.
//! 3. `pump` executes one or more ordered maintenance rounds (up to cap).
//! 4. Deferred messages, workspace edits, queued commands, and hook-generated invocations converge in-cycle when budget permits.
//! 5. Runtime emits `LoopDirective` for frontend scheduling and rendering.
//!
//! # Lifecycle
//!
//! * Startup: frontends create editor and begin event/pump loop.
//! * Running: repeated `on_event` and optional direct `pump` calls progress deferred state.
//! * Shutdown: `LoopDirective::should_quit` terminates frontend loop.
//!
//! # Concurrency & ordering
//!
//! * Event handling and pump rounds run on the editor thread.
//! * Scheduler completions are drained under per-round budgets to bound latency.
//! * Nu hook eval scheduling is ordered before scheduler drain to permit same-cycle convergence.
//! * Overlay commit remains serialized through `pending_overlay_commit` and executes only in pump.
//!
//! # Failure modes & recovery
//!
//! * Filesystem worker lag: bounded filesystem pump budget; redraw requested when new data arrives.
//! * Nu hook failures: handled in invocation/nu pipeline; runtime stays live.
//! * Workspace edit apply failures: surfaced as notifications; cycle continues.
//! * Scheduler backlog: bounded per-round drain plus round cap prevents unbounded pump stalls.
//!
//! # Recipes
//!
//! * Add new runtime event:
//!   1. Add variant to [`RuntimeEvent`].
//!   2. Route in `Editor::on_event`.
//!   3. Extend runtime invariants/tests for one-event-one-pump behavior.
//! * Add new pump phase:
//!   1. Add typed outcome in `pump::phases`.
//!   2. Insert phase in ordered round sequence in `pump::run_pump_cycle_with_report`.
//!   3. Update invariants/tests for order and continuation policy.

mod core;
pub(crate) mod pump;

pub use core::{CursorStyle, LoopDirective, RuntimeEvent};

#[cfg(test)]
use crate::Editor;

pub(crate) mod recorder;

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
