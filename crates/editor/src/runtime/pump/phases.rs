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

/// Outcome for deferred invocation drain phase.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DeferredInvocationsPhaseOutcome {
	pub(crate) drained_count: usize,
	pub(crate) should_quit: bool,
}

/// Maximum deferred invocations drained per round.
pub(crate) const MAX_DEFERRED_INVOCATIONS_PER_ROUND: usize = 32;

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
	if !allow_commit || !editor.state.frame.deferred_work.take_overlay_commit() {
		return OverlayCommitPhaseOutcome::default();
	}

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
		let edits = editor.state.frame.deferred_work.take_workspace_edits();
		if !edits.is_empty() {
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

pub(crate) async fn phase_drain_deferred_invocations(editor: &mut Editor, max: usize) -> DeferredInvocationsPhaseOutcome {
	let report = editor.drain_deferred_invocations_report(max).await;
	DeferredInvocationsPhaseOutcome {
		drained_count: report.drained_count,
		should_quit: report.should_quit,
	}
}
