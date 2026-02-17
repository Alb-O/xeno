use std::time::Duration;

use super::{CursorStyle, RuntimeEvent};

/// Monotonic token assigned to submitted runtime events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubmitToken(pub u64);

/// Origin marker for submitted frontend runtime events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeEventSource {
	Frontend,
	Replay,
	System,
}

/// Frontend runtime event envelope with monotonic sequence metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEventEnvelope {
	pub seq: u64,
	pub source: RuntimeEventSource,
	pub event: RuntimeEvent,
}

/// External runtime signal emitted by subsystem adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalEventKind {
	Wake,
	FilesystemChanged,
	SchedulerCompleted,
	RuntimeWorkQueued,
	QuitRequested,
}

/// External event envelope with monotonic sequence metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalEventEnvelope {
	pub seq: u64,
	pub kind: ExternalEventKind,
}

/// Frontend loop directive with causal metadata for event-driven dispatch.
#[derive(Debug, Clone, Copy)]
pub struct LoopDirectiveV2 {
	pub poll_timeout: Option<Duration>,
	pub needs_redraw: bool,
	pub cursor_style: CursorStyle,
	pub should_quit: bool,
	pub cause_seq: Option<u64>,
	pub drained_runtime_work: usize,
	pub pending_events: usize,
}

/// Runtime drain policy for event-driven coordinator processing.
#[derive(Debug, Clone, Copy)]
pub struct DrainPolicy {
	pub max_frontend_events: usize,
	pub max_external_events: usize,
	pub max_directives: usize,
	pub run_idle_maintenance: bool,
}

impl DrainPolicy {
	pub const fn for_on_event() -> Self {
		Self {
			max_frontend_events: 1,
			max_external_events: 0,
			max_directives: 1,
			run_idle_maintenance: false,
		}
	}

	pub const fn for_pump() -> Self {
		Self {
			max_frontend_events: 0,
			max_external_events: 0,
			max_directives: 1,
			run_idle_maintenance: true,
		}
	}
}

impl Default for DrainPolicy {
	fn default() -> Self {
		Self {
			max_frontend_events: 64,
			max_external_events: 64,
			max_directives: 64,
			run_idle_maintenance: true,
		}
	}
}

/// Drain progress report for event-driven runtime coordination.
#[derive(Debug, Clone, Default)]
pub struct DrainReport {
	pub handled_frontend_events: usize,
	pub handled_external_events: usize,
	pub directives_emitted: usize,
	pub reached_budget_cap: bool,
	pub last_directive: Option<LoopDirectiveV2>,
}
