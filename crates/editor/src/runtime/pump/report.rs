/// Maximum maintenance rounds executed by a single runtime maintenance cycle.
pub(crate) const MAX_PUMP_ROUNDS: usize = 3;

/// Ordered runtime maintenance phases executed inside each pump round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PumpPhase {
	UiTickAndTick,
	FilesystemEvents,
	DrainMessages,
	KickNuHookEval,
	DrainScheduler,
	DrainRuntimeWork,
}

/// Per-round progress flags used by bounded-convergence control flow.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RoundWorkFlags {
	pub(crate) filesystem_events: usize,
	pub(crate) drained_messages: usize,
	pub(crate) scheduler_completions: usize,
	pub(crate) drained_runtime_work: usize,
}

impl RoundWorkFlags {
	pub(crate) fn made_progress(self) -> bool {
		self.filesystem_events > 0 || self.drained_messages > 0 || self.scheduler_completions > 0 || self.drained_runtime_work > 0
	}
}

/// Report for one maintenance round.
#[derive(Debug, Clone, Default)]
pub(crate) struct RoundReport {
	pub(crate) phases: Vec<PumpPhase>,
	pub(crate) work: RoundWorkFlags,
}

/// Aggregate report for one runtime maintenance cycle.
#[derive(Debug, Clone, Default)]
pub(crate) struct PumpCycleReport {
	pub(crate) rounds_executed: usize,
	pub(crate) reached_round_cap: bool,
	pub(crate) should_quit: bool,
	pub(crate) rounds: Vec<RoundReport>,
}
