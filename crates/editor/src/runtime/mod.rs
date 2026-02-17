//! Frontend-agnostic runtime event loop and bounded-convergence pump pipeline.
//! Anchor ID: XENO_ANCHOR_RUNTIME_LOOP
//!
//! # Purpose
//!
//! * Defines the frontend/runtime event boundary (`RuntimeEvent`).
//! * Owns event-driven runtime coordination via `Editor::{submit_event,submit_external_event,drain_until_idle,poll_directive}`.
//! * Keeps compatibility wrappers `Editor::{on_event,pump}` that drain through the same coordinator.
//! * Produces `LoopDirectiveV2` (causal metadata) and compatibility `LoopDirective`.
//!
//! # Mental model
//!
//! * Frontends enqueue events/signals; `drain_until_idle` processes queued work under explicit `DrainPolicy` budgets.
//! * Each drained directive still runs ordered maintenance phases via bounded pump rounds (`MAX_PUMP_ROUNDS`).
//! * Compatibility `on_event` and `pump` call into the same coordinator with single-directive policies.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`RuntimeEvent`] | Frontend event payload | Must be translated into editor handlers before `pump` | frontend adapters |
//! | [`LoopDirective`] | Frontend control output | Must reflect redraw/quit after full cycle | `Editor::pump` |
//! | [`LoopDirectiveV2`] | Event-driven directive with causal metadata | Must preserve cause sequence and pending depth snapshots | `Editor::drain_until_idle` |
//! | [`DrainPolicy`] | Event-driven drain budget policy | Must bound frontend/external work and directive emission | runtime coordinator APIs |
//! | [`CursorStyle`] | Editor cursor intent | Must remain mode-consistent unless UI explicitly overrides | `Editor::derive_cursor_style` |
//! | [`work_queue::RuntimeWorkQueue`] | Runtime-owned deferred work queue | Overlay commits/workspace edits/invocations must be queued through this queue and drained only in pump phases | input/effects/message producers and `pump::phases` |
//! | [`pump::PumpCycleReport`] | Internal round/phase progress report | Must preserve phase order and cap tracking for invariants/tests | `pump::run_pump_cycle_with_report` |
//! | [`pump::RoundWorkFlags`] | Per-round progress summary | Must drive bounded-convergence continuation policy | `pump::run_pump_cycle_with_report` |
//!
//! # Invariants
//!
//! * `submit_event` sequence IDs must remain monotonic for queued envelopes.
//! * `on_event` compatibility path must execute exactly one maintenance directive after one queued event.
//! * Each pump cycle must execute no more than `pump::MAX_PUMP_ROUNDS` rounds.
//! * Each round must preserve phase ordering: tick -> filesystem -> messages -> Nu kick -> scheduler -> runtime work.
//! * Pump must kick queued Nu hook eval before scheduler drain in each round.
//! * Deferred runtime work producers must queue through `work_queue::RuntimeWorkQueue`.
//! * Deferred overlay commit work must be applied during `pump`, not during key handling.
//! * Deferred invocation/runtime work drain must preserve global FIFO order across work kinds.
//! * Runtime work drain must remain bounded by `phases::MAX_RUNTIME_WORK_ITEMS_PER_ROUND`.
//! * Runtime must return immediate quit directive when runtime work drain requests quit.
//! * Cursor style must default to insert beam vs non-insert block when UI has no override.
//!
//! # Data flow
//!
//! 1. Frontend submits `RuntimeEvent` or subsystem adapters submit `ExternalEventKind`.
//! 2. Runtime kernel assigns monotonic sequence IDs and queues envelopes.
//! 3. `drain_until_idle` pops queued envelopes under `DrainPolicy`.
//! 4. For each drained envelope, runtime routes input/external effects, then executes one bounded maintenance cycle.
//! 5. Runtime emits `LoopDirectiveV2` (or compatibility `LoopDirective`) for frontend scheduling/rendering.
//!
//! # Lifecycle
//!
//! * Startup: frontends create editor and begin submit/drain/poll loop (or compatibility on_event/pump loop).
//! * Running: repeated drain calls progress deferred state under explicit budgets.
//! * Shutdown: `LoopDirective::should_quit` terminates frontend loop.
//!
//! # Concurrency & ordering
//!
//! * Event handling and pump rounds run on the editor thread.
//! * Scheduler completions are drained under per-round budgets to bound latency.
//! * Nu hook eval scheduling is ordered before scheduler drain to permit same-cycle convergence.
//! * Overlay commit remains serialized through runtime work queue and executes only in pump.
//! * Runtime work is drained in FIFO order from a single queue.
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
//! * Add new runtime work kind:
//!   1. Extend [`work_queue::RuntimeWorkKind`].
//!   2. Handle it in `Editor::drain_runtime_work_report`.
//!   3. Update invariants/tests for order and continuation policy.

mod core;
pub(crate) mod kernel;
mod protocol;
pub(crate) mod pump;
mod work_drain;
pub(crate) mod work_queue;

pub use core::{CursorStyle, LoopDirective, RuntimeEvent};

pub use protocol::{
	DrainPolicy, DrainReport, ExternalEventEnvelope, ExternalEventKind, LoopDirectiveV2, RuntimeEventEnvelope, RuntimeEventSource, SubmitToken,
};

#[cfg(test)]
use crate::Editor;

pub(crate) mod recorder;

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
