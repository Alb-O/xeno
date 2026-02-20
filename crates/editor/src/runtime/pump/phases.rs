use std::time::Duration;

use xeno_primitives::Mode;

use crate::runtime::facade::{RuntimeFilesystemPort, RuntimeMessagePort, RuntimeOverlayPort, RuntimePorts, RuntimeSchedulerPort};
use crate::runtime::work_queue::RuntimeWorkKindCounts;

/// Outcome for filesystem service event-drain phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FilesystemPhaseOutcome {
	pub(crate) drained_events: usize,
}

/// Outcome for message-drain phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MessageDrainPhaseOutcome {
	pub(crate) drained_count: usize,
}

/// Outcome for scheduler completion drain phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SchedulerDrainPhaseOutcome {
	pub(crate) completed: usize,
}

/// Outcome for runtime work drain phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RuntimeWorkPhaseOutcome {
	pub(crate) drained_count: usize,
	pub(crate) drained_by_kind: RuntimeWorkKindCounts,
	pub(crate) should_quit: bool,
}

/// Maximum deferred runtime work items drained per round.
pub(crate) const MAX_RUNTIME_WORK_ITEMS_PER_ROUND: usize = 32;

/// Runtime policy constants.
const DRAIN_BUDGET_FAST: crate::scheduler::DrainBudget = crate::scheduler::DrainBudget {
	duration: Duration::from_millis(1),
	max_completions: 32,
};
const DRAIN_BUDGET_SLOW: crate::scheduler::DrainBudget = crate::scheduler::DrainBudget {
	duration: Duration::from_millis(3),
	max_completions: 64,
};

pub(crate) fn phase_ui_tick_and_editor_tick(ports: &mut RuntimePorts<'_>) {
	ports.ui_tick_and_editor_tick();
}

pub(crate) fn phase_filesystem_events(ports: &mut RuntimePorts<'_>) -> FilesystemPhaseOutcome {
	let drained_events = RuntimeFilesystemPort::drain_filesystem_events(ports);
	if drained_events > 0 {
		RuntimeFilesystemPort::refresh_file_picker(ports);
		RuntimeFilesystemPort::request_redraw(ports);
	}

	FilesystemPhaseOutcome { drained_events }
}

pub(crate) fn phase_drain_messages(ports: &mut RuntimePorts<'_>) -> MessageDrainPhaseOutcome {
	let report = RuntimeMessagePort::drain_messages(ports);
	if report.dirty.needs_redraw() {
		RuntimeMessagePort::request_redraw(ports);
	}

	MessageDrainPhaseOutcome {
		drained_count: report.drained_count,
	}
}

pub(crate) fn phase_kick_nu_hook_eval(ports: &mut RuntimePorts<'_>) {
	ports.kick_nu_hook_eval();
}

pub(crate) async fn phase_drain_scheduler(ports: &mut RuntimePorts<'_>) -> SchedulerDrainPhaseOutcome {
	let drain_budget = if matches!(RuntimeSchedulerPort::scheduler_mode(ports), Mode::Insert) {
		DRAIN_BUDGET_FAST
	} else {
		DRAIN_BUDGET_SLOW
	};

	let drain_stats = RuntimeSchedulerPort::drain_scheduler_budget(ports, drain_budget).await;
	RuntimeSchedulerPort::record_scheduler_metrics(ports, &drain_stats);
	RuntimeSchedulerPort::emit_scheduler_panic_notification(ports, &drain_stats);

	SchedulerDrainPhaseOutcome {
		completed: drain_stats.completed as usize,
	}
}

pub(crate) async fn phase_drain_runtime_work(ports: &mut RuntimePorts<'_>, max: usize) -> RuntimeWorkPhaseOutcome {
	let report = RuntimeOverlayPort::drain_runtime_work(ports, max).await;
	RuntimeWorkPhaseOutcome {
		drained_count: report.drained_count,
		drained_by_kind: report.drained_by_kind,
		should_quit: report.should_quit,
	}
}
