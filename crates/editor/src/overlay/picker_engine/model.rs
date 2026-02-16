//! Data model for picker intent and actions.

/// Decision produced by picker commit planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitDecision {
	/// Commit typed input exactly as-is.
	CommitTyped,
	/// Apply selected completion and keep picker open.
	ApplySelectionThenStay,
}

/// Key-driven picker actions independent from specific providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerAction {
	MoveSelection { delta: isize },
	PageSelection { direction: isize },
	ApplySelection,
	Commit(CommitDecision),
}
