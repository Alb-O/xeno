//! Undo/redo result types.

/// Result of an undo/redo operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryResult {
	/// The operation completed successfully.
	Success,
	/// No undo states available in the history.
	NothingToUndo,
	/// No redo states available in the history.
	NothingToRedo,
}
