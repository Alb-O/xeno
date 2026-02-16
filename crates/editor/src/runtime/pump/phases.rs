use std::time::Duration;

use xeno_primitives::Mode;

use crate::Editor;

/// Outcome for filesystem service pump phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FilesystemPhaseOutcome {
	pub(crate) changed: bool,
}

/// Outcome for deferred overlay commit phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OverlayCommitPhaseOutcome {
	pub(crate) committed: bool,
}

/// Outcome for message-drain phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MessageDrainPhaseOutcome {
	pub(crate) drained_count: usize,
}

/// Outcome for pending-workspace-edit apply phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WorkspaceEditsPhaseOutcome {
	pub(crate) applied_count: usize,
}

/// Outcome for scheduler completion drain phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SchedulerDrainPhaseOutcome {
	pub(crate) completed: usize,
}

/// Outcome for queued-command drain phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CommandQueuePhaseOutcome {
	pub(crate) executed_count: usize,
	pub(crate) should_quit: bool,
}

/// Outcome for hook-produced invocation drain phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NuHookDrainPhaseOutcome {
	pub(crate) drained_count: usize,
	pub(crate) should_quit: bool,
}

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

pub(crate) fn phase_filesystem_pump(editor: &mut Editor) -> FilesystemPhaseOutcome {
	let changed = editor.state.filesystem.pump(crate::filesystem::PumpBudget {
		max_index_msgs: 32,
		max_search_msgs: 8,
		max_time: Duration::from_millis(4),
	});
	if changed {
		editor.interaction_refresh_file_picker();
		editor.frame_mut().needs_redraw = true;
	}

	FilesystemPhaseOutcome { changed }
}

pub(crate) async fn phase_overlay_commit_if_pending(editor: &mut Editor, allow_commit: bool) -> OverlayCommitPhaseOutcome {
	if !allow_commit || !editor.state.frame.pending_overlay_commit {
		return OverlayCommitPhaseOutcome::default();
	}

	editor.state.frame.pending_overlay_commit = false;
	editor.interaction_commit().await;
	OverlayCommitPhaseOutcome { committed: true }
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

pub(crate) async fn phase_apply_workspace_edits(editor: &mut Editor) -> WorkspaceEditsPhaseOutcome {
	#[cfg(feature = "lsp")]
	{
		if !editor.state.frame.pending_workspace_edits.is_empty() {
			let edits = std::mem::take(&mut editor.state.frame.pending_workspace_edits);
			let applied_count = edits.len();
			for edit in edits {
				if let Err(err) = editor.apply_workspace_edit(edit).await {
					editor.notify(xeno_registry::notifications::keys::error(err.to_string()));
				}
			}
			editor.frame_mut().needs_redraw = true;
			return WorkspaceEditsPhaseOutcome { applied_count };
		}
	}

	#[cfg(not(feature = "lsp"))]
	{
		let _ = editor;
	}

	WorkspaceEditsPhaseOutcome::default()
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

	SchedulerDrainPhaseOutcome {
		completed: drain_stats.completed as usize,
	}
}

pub(crate) async fn phase_drain_command_queue(editor: &mut Editor) -> CommandQueuePhaseOutcome {
	let report = editor.drain_command_queue_report().await;
	CommandQueuePhaseOutcome {
		executed_count: report.executed_count,
		should_quit: report.should_quit,
	}
}

pub(crate) async fn phase_drain_nu_hook_invocations(editor: &mut Editor, max: usize) -> NuHookDrainPhaseOutcome {
	let report = editor.drain_nu_hook_invocations_report(max).await;
	NuHookDrainPhaseOutcome {
		drained_count: report.drained_count,
		should_quit: report.should_quit,
	}
}
