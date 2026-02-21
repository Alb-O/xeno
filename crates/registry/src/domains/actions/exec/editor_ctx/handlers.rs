/// Outcome of handling an action result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleOutcome {
	/// Result was handled, continue running.
	Handled,
	/// Result was handled, editor should quit.
	Quit,
}
