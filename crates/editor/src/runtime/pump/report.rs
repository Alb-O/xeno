/// Maximum maintenance rounds executed by a single `pump()` call.
pub(crate) const MAX_PUMP_ROUNDS: usize = 3;

/// Ordered runtime maintenance phases executed inside each pump round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PumpPhase {
	UiTickAndTick,
	FilesystemPump,
	OverlayCommit,
	DrainMessages,
	ApplyWorkspaceEdits,
	KickNuHookEval,
	DrainScheduler,
	DrainDeferredInvocations,
}

/// Per-round progress flags used by bounded-convergence control flow.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RoundWorkFlags {
	pub(crate) filesystem_changed: bool,
	pub(crate) overlay_commit_applied: bool,
	pub(crate) drained_messages: usize,
	pub(crate) applied_workspace_edits: usize,
	pub(crate) scheduler_completions: usize,
	pub(crate) drained_deferred_invocations: usize,
}

impl RoundWorkFlags {
	pub(crate) fn made_progress(self) -> bool {
		self.filesystem_changed
			|| self.overlay_commit_applied
			|| self.drained_messages > 0
			|| self.applied_workspace_edits > 0
			|| self.scheduler_completions > 0
			|| self.drained_deferred_invocations > 0
	}
}

/// Report for one maintenance round.
#[derive(Debug, Clone, Default)]
pub(crate) struct RoundReport {
	pub(crate) phases: Vec<PumpPhase>,
	pub(crate) work: RoundWorkFlags,
}

/// Aggregate report for one pump cycle.
#[derive(Debug, Clone, Default)]
pub(crate) struct PumpCycleReport {
	pub(crate) rounds_executed: usize,
	pub(crate) reached_round_cap: bool,
	pub(crate) should_quit: bool,
	pub(crate) rounds: Vec<RoundReport>,
}
