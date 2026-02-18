use std::time::Duration;

use xeno_primitives::Mode;

use crate::Editor;

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
	pub(crate) panicked: u64,
}

/// Outcome for runtime work drain phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RuntimeWorkPhaseOutcome {
	pub(crate) drained_count: usize,
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

pub(crate) fn phase_ui_tick_and_editor_tick(editor: &mut Editor) {
	editor.ui_tick();
	editor.tick();
}

pub(crate) fn phase_filesystem_events(editor: &mut Editor) -> FilesystemPhaseOutcome {
	let drained_events = editor.state.filesystem.drain_events();
	if drained_events > 0 {
		editor.interaction_refresh_file_picker();
		editor.frame_mut().needs_redraw = true;
	}

	FilesystemPhaseOutcome { drained_events }
}

pub(crate) fn phase_drain_messages(editor: &mut Editor) -> MessageDrainPhaseOutcome {
	let report = editor.drain_messages_report();
	if report.dirty.needs_redraw() {
		editor.frame_mut().needs_redraw = true;
	}

	MessageDrainPhaseOutcome {
		drained_count: report.drained_count,
	}
}

pub(crate) fn phase_kick_nu_hook_eval(editor: &mut Editor) {
	editor.kick_nu_hook_eval();
}

pub(crate) async fn phase_drain_scheduler(editor: &mut Editor) -> SchedulerDrainPhaseOutcome {
	let drain_budget = if matches!(editor.mode(), Mode::Insert) {
		DRAIN_BUDGET_FAST
	} else {
		DRAIN_BUDGET_SLOW
	};

	let drain_stats = editor.work_scheduler_mut().drain_budget(drain_budget).await;
	editor.metrics().record_hook_tick(drain_stats.completed, drain_stats.pending);
	editor
		.metrics()
		.record_worker_drain(drain_stats.completed, drain_stats.panicked, drain_stats.cancelled);

	if drain_stats.panicked > 0 {
		use xeno_registry::notifications::{AutoDismiss, Level, Notification};
		let message = if let Some(sample) = &drain_stats.panic_sample {
			format!("worker tasks panicked: {} (first: {}) (see logs)", drain_stats.panicked, sample)
		} else {
			format!("worker tasks panicked: {} (see logs)", drain_stats.panicked)
		};
		editor.show_notification(Notification::new("xeno-editor::worker_task_panic", Level::Error, AutoDismiss::DEFAULT, message));
	}

	SchedulerDrainPhaseOutcome {
		completed: drain_stats.completed as usize,
		panicked: drain_stats.panicked,
	}
}

pub(crate) async fn phase_drain_runtime_work(editor: &mut Editor, max: usize) -> RuntimeWorkPhaseOutcome {
	let report = editor.drain_runtime_work_report(max).await;
	RuntimeWorkPhaseOutcome {
		drained_count: report.drained_count,
		should_quit: report.should_quit,
	}
}
