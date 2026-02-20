//! Frontend-agnostic runtime event loop and bounded-convergence maintenance pipeline.
//! Anchor ID: XENO_ANCHOR_RUNTIME_LOOP
//!
//! # Purpose
//!
//! * Defines the frontend/runtime event boundary (`RuntimeEvent`).
//! * Owns event-driven runtime coordination via `Editor::{submit_event,drain_until_idle,poll_directive}`.
//! * Produces `LoopDirectiveV2` directives for frontend scheduling and render cadence.
//!
//! # Mental model
//!
//! * Frontends enqueue runtime events; `drain_until_idle` processes queued work under explicit `DrainPolicy` budgets.
//! * Each drained directive still runs ordered maintenance phases via bounded rounds (`MAX_PUMP_ROUNDS`).
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`RuntimeEvent`] | Frontend event payload | Must map to deterministic direct input application | frontend adapters |
//! | [`RuntimeCauseId`] | Monotonic causal chain identifier | Must propagate from runtime event envelopes to directives and deferred runtime work spawned by that event | runtime kernel + runtime work queue |
//! | [`LoopDirectiveV2`] | Event-driven directive with causal metadata | Must preserve cause sequence and pending depth snapshots | `Editor::drain_until_idle` |
//! | [`DrainPolicy`] | Event-driven drain budget policy | Must bound frontend work and directive emission | runtime coordinator APIs |
//! | [`RuntimeDrainStats`] | Runtime drain observability payload | Must report phase depth snapshots, per-kind drained counts, oldest age by kind, and exit reasons | `Editor::drain_until_idle` |
//! | [`CursorStyle`] | Editor cursor intent | Must remain mode-consistent unless UI explicitly overrides | `Editor::derive_cursor_style` |
//! | [`work_queue::RuntimeWorkQueue`] | Runtime-owned deferred work queue | Overlay commits/workspace edits/invocations must be queued through this queue and drained only in pump phases | input/effects/message producers and `pump::phases` |
//! | [`facade::RuntimePorts`] | Runtime subsystem mutation facade aggregate | Pump phases must mutate filesystem/scheduler/messages/runtime-work through facade traits | `pump::run_pump_cycle_with_report`, `pump::phases` |
//! | [`pump::PumpCycleReport`] | Internal round/phase progress report | Must preserve phase order and cap tracking for invariants/tests | `pump::run_pump_cycle_with_report` |
//! | [`pump::RoundWorkFlags`] | Per-round progress summary | Must drive bounded-convergence continuation policy | `pump::run_pump_cycle_with_report` |
//!
//! # Invariants
//!
//! * `submit_event` sequence IDs must remain monotonic for queued envelopes.
//! * `submit_event` followed by `drain_until_idle(DrainPolicy::for_on_event())` must emit exactly one maintenance directive.
//! * Frontend runtime events must be applied directly in-order through `Editor::apply_runtime_event_input`.
//! * Each maintenance cycle must execute no more than `pump::MAX_PUMP_ROUNDS` rounds.
//! * Each round must preserve phase ordering: tick -> filesystem -> messages -> Nu kick -> scheduler -> runtime work.
//! * Maintenance cycle must kick queued Nu hook eval before scheduler drain in each round.
//! * Deferred runtime work producers must queue through `work_queue::RuntimeWorkQueue`.
//! * Deferred overlay commit work must be applied during `drain_until_idle`, not during key handling.
//! * Deferred invocation/runtime work drain must preserve global FIFO order across work kinds.
//! * Runtime work drain must remain bounded by `phases::MAX_RUNTIME_WORK_ITEMS_PER_ROUND`.
//! * Runtime must return immediate quit directive when runtime work drain requests quit.
//! * Runtime causal metadata must propagate from submitted events to emitted directives and deferred runtime work spawned by that event.
//! * Runtime work fairness must stay bounded: no queued work kind may starve indefinitely while other kinds continue draining across bounded rounds.
//! * Log-only runtime command-path drains must emit unknown-command notifications through the canonical notification key path.
//! * Pump phase mutations must cross explicit runtime facade traits rather than direct `EditorState` field reads.
//! * Editor/runtime construction must not require an already-active Tokio runtime.
//! * Cursor style must default to insert beam vs non-insert block when UI has no override.
//! * Must assign distinct, ordered cause IDs to directives from separately drained events.
//! * Must propagate cause ID from draining work to follow-up work enqueued during that drain.
//! * Must no-op overlay commit when the overlay was cancelled before drain.
//! * Must no-op overlay commit when the overlay was force-closed before drain.
//! * Must commit only the first queued overlay commit; subsequent commits are no-ops.
//!
//! # Data flow
//!
//! 1. Frontend submits `RuntimeEvent`.
//! 2. Runtime kernel assigns monotonic sequence IDs and queues envelopes.
//! 3. `drain_until_idle` pops queued envelopes under `DrainPolicy`.
//! 4. For each drained envelope, runtime applies input effects, then executes one bounded maintenance cycle.
//! 5. Runtime emits `LoopDirectiveV2` for frontend scheduling/rendering.
//!
//! # Lifecycle
//!
//! * Startup: frontends create editor and begin submit/drain/poll loop.
//! * Running: repeated drain calls progress deferred state under explicit budgets.
//! * Shutdown: `LoopDirectiveV2::should_quit` terminates frontend loop.
//!
//! # Concurrency & ordering
//!
//! * Event handling and maintenance rounds run on the editor thread.
//! * Scheduler completions are drained under per-round budgets to bound latency.
//! * Nu hook eval scheduling is ordered before scheduler drain to permit same-cycle convergence.
//! * Overlay commit remains serialized through runtime work queue and executes only during runtime drain phases.
//! * Runtime work is drained in FIFO order from a single queue.
//!
//! # Failure modes & recovery
//!
//! * Filesystem worker lag: bounded filesystem phase budget; redraw requested when new data arrives.
//! * Nu hook failures: handled in invocation/nu pipeline; runtime stays live.
//! * Workspace edit apply failures: surfaced as notifications; cycle continues.
//! * Scheduler backlog: bounded per-round drain plus round cap prevents unbounded runtime stalls.
//!
//! # Recipes
//!
//! * Add new runtime event:
//!   1. Add variant to [`RuntimeEvent`].
//!   2. Route in `Editor::apply_frontend_event_envelope`.
//!   3. Extend runtime invariants/tests for one-event-one-maintenance-cycle behavior.
//! * Add new runtime work kind:
//!   1. Extend [`work_queue::RuntimeWorkKind`].
//!   2. Handle it in `Editor::drain_runtime_work_report`.
//!   3. Update invariants/tests for order and continuation policy.

mod core;
pub(crate) mod facade;
pub(crate) mod kernel;
mod protocol;
pub(crate) mod pump;
mod work_drain;
pub(crate) mod work_queue;

pub use core::{CursorStyle, RuntimeEvent};

pub use protocol::{
	DrainPolicy, DrainReport, LoopDirectiveV2, RuntimeCauseId, RuntimeDrainExitReason, RuntimeDrainStats, RuntimeEventEnvelope, RuntimeEventSource,
	RuntimePhaseQueueDepthSnapshot, SubmitToken,
};

#[cfg(test)]
use crate::Editor;

pub(crate) mod recorder;

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
