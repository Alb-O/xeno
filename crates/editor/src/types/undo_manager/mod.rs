//! Editor-level undo manager with host trait abstraction.
//!
//! The [`UndoManager`] centralizes undo/redo stack management and provides
//! a prepare/finalize pattern for edit operations. The [`UndoHost`] trait
//! abstracts the Editor operations needed for undo, enabling cleaner
//! separation of concerns.
//!
//! # Architecture
//!
//! ```text
//! UndoManager                    UndoHost (Editor implements)
//! ┌──────────────────┐           ┌────────────────────────────┐
//! │ undo_stack       │           │ collect_view_snapshots()   │
//! │ redo_stack       │◄─────────►│ restore_view_snapshots()   │
//! │                  │           │ undo_documents()           │
//! │ prepare_edit()   │           │ redo_documents()           │
//! │ finalize_edit()  │           │ notify_*()                 │
//! │ undo()           │           └────────────────────────────┘
//! │ redo()           │
//! └──────────────────┘
//! ```

use std::collections::HashMap;

use tracing::trace;
use xeno_primitives::{CommitResult, EditOrigin};

use super::{EditorUndoGroup, ViewSnapshot};
use crate::buffer::{DocumentId, ViewId};

/// Manages editor-level undo/redo stacks.
///
/// This component owns the undo and redo stacks and provides methods for:
/// - Preparing an edit (capturing pre-edit state)
/// - Finalizing an edit (pushing undo group if needed)
/// - Executing undo/redo operations
///
/// The actual document operations and view snapshot management are delegated
/// to the [`UndoHost`] trait, which the Editor implements.
#[derive(Debug, Default)]
pub struct UndoManager {
	/// Editor-level undo grouping stack.
	undo_stack: Vec<EditorUndoGroup>,
	/// Editor-level redo grouping stack.
	redo_stack: Vec<EditorUndoGroup>,
	#[cfg(test)]
	pub finalize_calls: usize,
}

/// Pre-edit state captured by [`UndoManager::prepare_edit`].
///
/// Holds all information needed to finalize an edit, including affected documents
/// and pre-edit view snapshots.
#[derive(Debug)]
pub struct PreparedEdit {
	/// Documents affected by this edit.
	pub affected_docs: Vec<DocumentId>,
	/// View snapshots captured before the edit.
	pub pre_views: HashMap<ViewId, ViewSnapshot>,
	/// Origin of this edit.
	pub origin: EditOrigin,
}

/// Trait for operations needed by [`UndoManager`].
///
/// Implemented by `EditorUndoHost` to provide view snapshot collection,
/// document-level undo/redo, and notifications.
pub trait UndoHost {
	/// Checks if editing is allowed (not readonly).
	fn guard_readonly(&mut self) -> bool;

	/// Returns the document ID for a buffer.
	fn doc_id_for_buffer(&self, buffer_id: ViewId) -> DocumentId;

	/// Collects view snapshots for all buffers viewing a document.
	fn collect_view_snapshots(&self, doc_id: DocumentId) -> HashMap<ViewId, ViewSnapshot>;

	/// Captures current view snapshots for all buffers viewing the given documents.
	fn capture_current_view_snapshots(&self, doc_ids: &[DocumentId]) -> HashMap<ViewId, ViewSnapshot>;

	/// Restores view snapshots to their corresponding buffers.
	fn restore_view_snapshots(&mut self, snapshots: &HashMap<ViewId, ViewSnapshot>);

	/// Undoes all documents in the list. Returns true if all succeeded.
	fn undo_documents(&mut self, doc_ids: &[DocumentId]) -> bool;

	/// Redoes all documents in the list. Returns true if all succeeded.
	fn redo_documents(&mut self, doc_ids: &[DocumentId]) -> bool;

	/// Notifies that undo was performed.
	fn notify_undo(&mut self);

	/// Notifies that redo was performed.
	fn notify_redo(&mut self);

	/// Notifies that there's nothing to undo.
	fn notify_nothing_to_undo(&mut self);

	/// Notifies that there's nothing to redo.
	fn notify_nothing_to_redo(&mut self);
}

impl UndoManager {
	/// Creates a new empty undo manager.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns the number of undo groups.
	pub fn undo_len(&self) -> usize {
		self.undo_stack.len()
	}

	/// Returns the number of redo groups.
	pub fn redo_len(&self) -> usize {
		self.redo_stack.len()
	}

	/// Returns `true` if there are undo groups.
	pub fn can_undo(&self) -> bool {
		!self.undo_stack.is_empty()
	}

	/// Returns `true` if there are redo groups.
	pub fn can_redo(&self) -> bool {
		!self.redo_stack.is_empty()
	}

	/// Returns a reference to the last undo group, if any.
	pub fn last_undo_group(&self) -> Option<&EditorUndoGroup> {
		self.undo_stack.last()
	}

	/// Returns a reference to the last redo group, if any.
	pub fn last_redo_group(&self) -> Option<&EditorUndoGroup> {
		self.redo_stack.last()
	}

	/// Pushes an undo group directly and clears the redo stack.
	///
	/// For use by subsystems (e.g., LSP workspace edits) that manage their
	/// own transaction application outside the prepare/finalize cycle.
	pub fn push_group(&mut self, group: EditorUndoGroup) {
		self.redo_stack.clear();
		self.push_undo_group(group);
	}

	/// Pushes an undo group without clearing the redo stack.
	///
	/// Used internally by [`Self::finalize_edit`] which handles redo clearing
	/// separately to ensure it happens on all applied edits (even merged ones).
	fn push_undo_group(&mut self, group: EditorUndoGroup) {
		trace!(
			docs = ?group.affected_docs,
			origin = ?group.origin,
			snapshots = group.view_snapshots.len(),
			undo_stack = self.undo_stack.len() + 1,
			"undo group pushed"
		);
		self.undo_stack.push(group);
	}

	/// Prepares an edit operation by capturing pre-edit state.
	///
	/// Should be called before applying a transaction.
	pub fn prepare_edit(&self, host: &impl UndoHost, buffer_id: ViewId, origin: EditOrigin) -> PreparedEdit {
		let doc_id = host.doc_id_for_buffer(buffer_id);
		let pre_views = host.collect_view_snapshots(doc_id);

		PreparedEdit {
			affected_docs: vec![doc_id],
			pre_views,
			origin,
		}
	}

	/// Finalizes an edit operation based on the actual commit outcome.
	///
	/// Clears the editor redo stack on any successful edit (even merged),
	/// and pushes a new undo group only when a new document-level undo step
	/// was created.
	pub fn finalize_edit(&mut self, result: &CommitResult, prep: PreparedEdit) {
		#[cfg(test)]
		{
			self.finalize_calls += 1;
		}

		if result.applied {
			self.redo_stack.clear();

			if result.undo_recorded {
				self.push_undo_group(EditorUndoGroup {
					affected_docs: prep.affected_docs,
					view_snapshots: prep.pre_views,
					origin: prep.origin,
				});
			}
		}
	}

	/// Executes a closure as an undoable edit operation.
	pub fn with_edit<H, F>(&mut self, host: &mut H, buffer_id: ViewId, origin: EditOrigin, apply: F) -> bool
	where
		H: UndoHost,
		F: FnOnce(&mut H) -> CommitResult,
	{
		let prep = self.prepare_edit(host, buffer_id, origin);
		let result = apply(host);
		let applied = result.applied;
		self.finalize_edit(&result, prep);
		applied
	}

	/// Executes a closure with access to the undo manager and host.
	pub fn with_undo_redo<H, F>(&mut self, host: &mut H, f: F)
	where
		H: UndoHost,
		F: FnOnce(&mut UndoManager, &mut H),
	{
		f(self, host);
	}

	/// Undoes the last change, restoring view state for all affected buffers.
	pub fn undo(&mut self, host: &mut impl UndoHost) -> bool {
		let span = tracing::trace_span!(
			target: "xeno_undo_trace",
			"undo_manager.undo",
			undo_depth = self.undo_stack.len(),
			redo_depth = self.redo_stack.len()
		);
		let _span_guard = span.enter();
		if !host.guard_readonly() {
			trace!(target: "xeno_undo_trace", result = "readonly_blocked");
			return false;
		}

		let Some(group) = self.undo_stack.pop() else {
			trace!(target: "xeno_undo_trace", result = "nothing_to_undo");
			host.notify_nothing_to_undo();
			return false;
		};
		trace!(
			target: "xeno_undo_trace",
			affected_docs = ?group.affected_docs,
			origin = ?group.origin,
			"undo_manager.group.popped"
		);

		let current_snapshots = host.capture_current_view_snapshots(&group.affected_docs);
		let ok = host.undo_documents(&group.affected_docs);
		trace!(target: "xeno_undo_trace", ok, "undo_manager.undo_documents.done");

		if ok {
			host.restore_view_snapshots(&group.view_snapshots);
			self.redo_stack.push(EditorUndoGroup {
				affected_docs: group.affected_docs,
				view_snapshots: current_snapshots,
				origin: group.origin,
			});
			host.notify_undo();
			trace!(
				target: "xeno_undo_trace",
				undo_depth = self.undo_stack.len(),
				redo_depth = self.redo_stack.len(),
				result = "ok"
			);
			true
		} else {
			self.undo_stack.push(group);
			host.notify_nothing_to_undo();
			trace!(
				target: "xeno_undo_trace",
				undo_depth = self.undo_stack.len(),
				redo_depth = self.redo_stack.len(),
				result = "undo_documents_failed"
			);
			false
		}
	}

	/// Redoes the last undone change, restoring view state for all affected buffers.
	pub fn redo(&mut self, host: &mut impl UndoHost) -> bool {
		let span = tracing::trace_span!(
			target: "xeno_undo_trace",
			"undo_manager.redo",
			undo_depth = self.undo_stack.len(),
			redo_depth = self.redo_stack.len()
		);
		let _span_guard = span.enter();
		if !host.guard_readonly() {
			trace!(target: "xeno_undo_trace", result = "readonly_blocked");
			return false;
		}

		let Some(group) = self.redo_stack.pop() else {
			trace!(target: "xeno_undo_trace", result = "nothing_to_redo");
			host.notify_nothing_to_redo();
			return false;
		};
		trace!(
			target: "xeno_undo_trace",
			affected_docs = ?group.affected_docs,
			origin = ?group.origin,
			"undo_manager.group.popped"
		);

		let current_snapshots = host.capture_current_view_snapshots(&group.affected_docs);
		let ok = host.redo_documents(&group.affected_docs);
		trace!(target: "xeno_undo_trace", ok, "undo_manager.redo_documents.done");

		if ok {
			host.restore_view_snapshots(&group.view_snapshots);
			self.undo_stack.push(EditorUndoGroup {
				affected_docs: group.affected_docs,
				view_snapshots: current_snapshots,
				origin: group.origin,
			});
			host.notify_redo();
			trace!(
				target: "xeno_undo_trace",
				undo_depth = self.undo_stack.len(),
				redo_depth = self.redo_stack.len(),
				result = "ok"
			);
			true
		} else {
			self.redo_stack.push(group);
			host.notify_nothing_to_redo();
			trace!(
				target: "xeno_undo_trace",
				undo_depth = self.undo_stack.len(),
				redo_depth = self.redo_stack.len(),
				result = "redo_documents_failed"
			);
			false
		}
	}
}

#[cfg(test)]
mod tests;
