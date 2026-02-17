use std::time::Duration;

/// Drain budget for bounded work convergence loops.
#[derive(Debug, Clone, Copy)]
pub struct DrainBudget {
	/// Maximum wall-clock duration to spend draining.
	pub duration: Duration,
	/// Maximum completed tasks/items to drain.
	pub max_completions: usize,
}

impl DrainBudget {
	/// Creates a new budget.
	pub const fn new(duration: Duration, max_completions: usize) -> Self {
		Self { duration, max_completions }
	}
}

impl Default for DrainBudget {
	fn default() -> Self {
		Self {
			duration: Duration::from_millis(4),
			max_completions: 64,
		}
	}
}

/// Drain report from one bounded convergence pass.
#[derive(Debug, Clone, Copy, Default)]
pub struct DrainReport {
	/// Number of completed tasks drained.
	pub completed: u64,
	/// Pending interactive work after drain.
	pub pending_interactive: usize,
	/// Pending background work after drain.
	pub pending_background: usize,
	/// Whether drain exited due to hitting budget limits.
	pub budget_exhausted: bool,
}
