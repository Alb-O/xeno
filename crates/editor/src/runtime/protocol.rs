use std::time::{Duration, Instant};

use super::work_queue::{RuntimeWorkKindCounts, RuntimeWorkKindOldestAgesMs};
use super::{CursorStyle, RuntimeEvent};

/// Monotonic token assigned to submitted runtime events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubmitToken(pub u64);

/// Monotonic cause identifier propagated across one runtime causal chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeCauseId(pub u64);

/// Runtime event source tag used for backpressure and observability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeEventSource {
	Frontend,
	Replay,
	Internal,
}

impl RuntimeEventSource {
	pub(crate) const fn idx(self) -> usize {
		match self {
			Self::Frontend => 0,
			Self::Replay => 1,
			Self::Internal => 2,
		}
	}
}

/// Frontend runtime event envelope with monotonic sequence metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEventEnvelope {
	pub seq: u64,
	pub cause_id: RuntimeCauseId,
	pub submitted_at: Instant,
	pub source: RuntimeEventSource,
	pub event: RuntimeEvent,
}

/// Frontend loop directive with causal metadata for event-driven dispatch.
#[derive(Debug, Clone, Copy)]
pub struct LoopDirectiveV2 {
	pub poll_timeout: Option<Duration>,
	pub needs_redraw: bool,
	pub cursor_style: CursorStyle,
	pub should_quit: bool,
	pub cause_seq: Option<u64>,
	pub cause_id: Option<RuntimeCauseId>,
	pub drained_runtime_work: usize,
	pub pending_events: usize,
}

/// Runtime drain policy for event-driven coordinator processing.
#[derive(Debug, Clone, Copy)]
pub struct DrainPolicy {
	pub max_frontend_events: usize,
	pub max_events_per_source: usize,
	pub max_directives: usize,
	pub run_idle_maintenance: bool,
}

impl DrainPolicy {
	pub const fn for_on_event() -> Self {
		Self {
			max_frontend_events: 1,
			max_events_per_source: 1,
			max_directives: 1,
			run_idle_maintenance: false,
		}
	}

	pub const fn for_pump() -> Self {
		Self {
			max_frontend_events: 0,
			max_events_per_source: 0,
			max_directives: 1,
			run_idle_maintenance: true,
		}
	}
}

impl Default for DrainPolicy {
	fn default() -> Self {
		Self {
			max_frontend_events: 64,
			max_events_per_source: 64,
			max_directives: 64,
			run_idle_maintenance: true,
		}
	}
}

/// Runtime drain exit reason for one maintenance round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeDrainExitReason {
	Idle,
	RoundCap,
	Quit,
	BudgetCap,
}

impl Default for RuntimeDrainExitReason {
	fn default() -> Self {
		Self::Idle
	}
}

/// Queue depth snapshot sampled after a runtime maintenance phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePhaseQueueDepthSnapshot {
	pub round_idx: usize,
	pub phase: &'static str,
	pub event_queue_depth: usize,
	pub work_queue_depth: usize,
}

/// Runtime drain observability payload emitted by `drain_until_idle`.
#[derive(Debug, Clone, Default)]
pub struct RuntimeDrainStats {
	pub rounds_executed: usize,
	pub final_event_queue_depth: usize,
	pub final_work_queue_depth: usize,
	pub phase_queue_depths: Vec<RuntimePhaseQueueDepthSnapshot>,
	pub drained_work_by_kind: RuntimeWorkKindCounts,
	pub oldest_work_age_ms: RuntimeWorkKindOldestAgesMs,
	pub round_exit_reasons: Vec<RuntimeDrainExitReason>,
}

/// Drain progress report for event-driven runtime coordination.
#[derive(Debug, Clone, Default)]
pub struct DrainReport {
	pub handled_frontend_events: usize,
	pub directives_emitted: usize,
	pub reached_budget_cap: bool,
	pub last_directive: Option<LoopDirectiveV2>,
	pub runtime_stats: RuntimeDrainStats,
}
