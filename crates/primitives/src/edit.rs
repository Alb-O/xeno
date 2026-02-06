//! Edit operation types: errors, policies, and results.
//!
//! These types form the foundation for a single, authoritative edit gate
//! that handles undo/redo, readonly checks, and syntax scheduling.

use crate::{Selection, Transaction};

/// Error type for edit operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EditError {
	#[error("document is read-only: {scope:?} ({reason:?})")]
	ReadOnly {
		scope: ReadOnlyScope,
		reason: ReadOnlyReason,
	},

	#[error("invalid selection: {0}")]
	InvalidSelection(String),

	#[error("transaction apply failed: {0}")]
	ApplyFailed(String),

	#[error("undo/redo unavailable: {0}")]
	History(String),

	#[error("internal: {0}")]
	Internal(String),
}

/// Scope at which read-only restriction applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyScope {
	/// Read-only flag on the buffer view.
	Buffer,
	/// Read-only flag on the underlying document.
	Document,
}

/// Reason why a document or buffer is read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOnlyReason {
	/// Explicitly flagged as read-only.
	FlaggedReadOnly,
	/// File system permission denied.
	PermissionDenied,
	/// Buffer-local override.
	BufferOverride,
	/// Reason not specified.
	Unknown,
}

/// Policy for recording undo history during an edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UndoPolicy {
	/// Do not record undo (rare; e.g., ephemeral or preview edits).
	NoUndo,
	/// Normal: this commit becomes an undo step.
	#[default]
	Record,
	/// Merge into current group (e.g., insert-typing run).
	MergeWithCurrentGroup,
	/// Explicit boundary: end current group and start a new one.
	Boundary,
}

/// Policy for syntax tree updates during an edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyntaxPolicy {
	/// Do not touch syntax (rare; internal ops).
	None,
	/// Mark dirty; do work lazily (e.g., next render).
	#[default]
	MarkDirty,
	/// Apply incremental update if available; else mark dirty.
	IncrementalOrDirty,
	/// Force immediate full reparse (used for undo/redo, explicit operations).
	FullReparseNow,
}

/// Result of a successful document commit.
///
/// Bundles the outcomes of a modification, including version updates,
/// affected ranges, and syntax handling status.
#[derive(Debug, Clone)]
pub struct CommitResult {
	/// Whether the edit was actually applied to the document.
	pub applied: bool,
	/// Document version immediately before the edit.
	pub version_before: u64,
	/// Document version after the edit was applied.
	pub version_after: u64,
	/// Selection state override requested by the edit planner.
	pub selection_after: Option<Selection>,
	/// Whether a new logical undo step was created.
	///
	/// `false` if the edit was merged into an existing group or not recorded.
	pub undo_recorded: bool,
}

impl CommitResult {
	/// Creates a stub result for migration or testing.
	pub fn stub(version: u64) -> Self {
		Self {
			applied: true,
			version_before: version,
			version_after: version.checked_add(1).expect("document version overflow"),
			selection_after: None,
			undo_recorded: true,
		}
	}

	/// Creates a result for an edit blocked by a readonly check.
	pub fn blocked(version: u64) -> Self {
		Self {
			applied: false,
			version_before: version,
			version_after: version,
			selection_after: None,
			undo_recorded: false,
		}
	}
}

/// A complete edit commit request.
///
/// Bundles a transaction with policies for history recording, syntax updates,
/// and metadata about the edit's origin.
#[derive(Debug, Clone)]
pub struct EditCommit {
	/// The transaction containing the text changes.
	pub tx: Transaction,
	/// Policy for recording history.
	pub undo: UndoPolicy,
	/// Policy for updating syntax highlighting.
	pub syntax: SyntaxPolicy,
	/// Origin of this edit (for grouping or debugging).
	pub origin: EditOrigin,
	/// Optional selection override produced by the planner.
	pub selection_after: Option<Selection>,
}

impl EditCommit {
	/// Creates a new edit commit with default policies.
	pub fn new(tx: Transaction) -> Self {
		Self {
			tx,
			undo: UndoPolicy::default(),
			syntax: SyntaxPolicy::default(),
			origin: EditOrigin::Internal("unspecified"),
			selection_after: None,
		}
	}

	/// Sets the undo policy.
	pub fn with_undo(mut self, policy: UndoPolicy) -> Self {
		self.undo = policy;
		self
	}

	/// Sets the syntax policy.
	pub fn with_syntax(mut self, policy: SyntaxPolicy) -> Self {
		self.syntax = policy;
		self
	}

	/// Sets the edit origin.
	pub fn with_origin(mut self, origin: EditOrigin) -> Self {
		self.origin = origin;
		self
	}

	/// Sets the selection after the edit.
	pub fn with_selection(mut self, selection: Selection) -> Self {
		self.selection_after = Some(selection);
		self
	}
}

/// Origin of an edit operation.
///
/// Useful for grouping related edits, telemetry, and debugging.
#[derive(Debug, Clone)]
pub enum EditOrigin {
	/// Edit from an EditOp (data-oriented edit operation).
	EditOp { id: &'static str },
	/// Edit from an ex-mode command.
	Command { name: String },
	/// Edit from macro replay.
	MacroReplay,
	/// Edit from LSP (code action, rename, format, etc.).
	Lsp,
	/// Internal edit (undo/redo replay, etc.).
	Internal(&'static str),
}
