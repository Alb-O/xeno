//! Edit policy types for configuring transaction behavior.

use xeno_primitives::{EditOrigin, UndoPolicy};

/// Policy for applying an edit transaction.
///
/// Combines the undo recording policy with the edit origin for tracing.
#[derive(Debug, Clone)]
pub struct ApplyEditPolicy {
	/// How to record this edit for undo.
	pub undo: UndoPolicy,
	/// Origin of this edit for debugging/tracing.
	pub origin: EditOrigin,
}

impl ApplyEditPolicy {
	/// Creates a policy that records the edit for undo.
	pub fn record(origin: EditOrigin) -> Self {
		Self {
			undo: UndoPolicy::Record,
			origin,
		}
	}

	/// Creates a policy that merges with the current undo group (for insert mode).
	pub fn merge(origin: EditOrigin) -> Self {
		Self {
			undo: UndoPolicy::MergeWithCurrentGroup,
			origin,
		}
	}

	/// Creates a policy that skips undo recording.
	pub fn no_undo(origin: EditOrigin) -> Self {
		Self {
			undo: UndoPolicy::NoUndo,
			origin,
		}
	}
}
