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

use std::collections::{HashMap, HashSet};

use tracing::trace;
use xeno_primitives::{CommitResult, EditOrigin, UndoPolicy};

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
	/// Pending deferred undo/redo operation.
	pending_history: Option<PendingHistoryOp>,
	#[cfg(test)]
	pub finalize_calls: usize,
}

/// Pre-edit state captured by [`UndoManager::prepare_edit`].
///
/// This struct holds all the information needed to finalize an edit:
/// - Which documents are affected
/// - View snapshots from before the edit
/// - Whether this edit should start a new undo group
/// - The origin of the edit
#[derive(Debug)]
pub struct PreparedEdit {
	/// Documents affected by this edit.
	pub affected_docs: Vec<DocumentId>,
	/// View snapshots captured before the edit.
	pub pre_views: HashMap<ViewId, ViewSnapshot>,
	/// Whether this edit should start a new undo group.
	pub start_new_group: bool,
	/// Origin of this edit.
	pub origin: EditOrigin,
}

/// Kind of remote history operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryKind {
	Undo,
	Redo,
}

#[derive(Debug)]
struct PendingHistoryOp {
	kind: HistoryKind,
	group: EditorUndoGroup,
	current_snapshots: HashMap<ViewId, ViewSnapshot>,
	remaining_docs: HashSet<DocumentId>,
	restore_stack: bool,
}

/// Trait for operations needed by [`UndoManager`].
///
/// Implemented by [`EditorUndoHost`] to provide:
/// - Read-only access to guard conditions and document state
/// - View snapshot collection and restoration
/// - Document-level undo/redo operations
/// - Notifications for undo/redo events
///
/// This abstraction allows UndoManager to operate without direct Editor access,
/// making the undo logic more testable and the dependencies more explicit.
///
/// [`EditorUndoHost`]: crate::impls::undo_host::EditorUndoHost
pub trait UndoHost {
	/// Checks if the buffer is readonly and shows notification if so.
	/// Returns `true` if editing is allowed.
	fn guard_readonly(&mut self) -> bool;

	/// Returns the document ID for a buffer.
	fn doc_id_for_buffer(&self, buffer_id: ViewId) -> DocumentId;

	/// Collects view snapshots for all buffers viewing a document.
	fn collect_view_snapshots(&self, doc_id: DocumentId) -> HashMap<ViewId, ViewSnapshot>;

	/// Captures current view snapshots for all buffers viewing the given documents.
	fn capture_current_view_snapshots(
		&self,
		doc_ids: &[DocumentId],
	) -> HashMap<ViewId, ViewSnapshot>;

	/// Restores view snapshots to their corresponding buffers.
	fn restore_view_snapshots(&mut self, snapshots: &HashMap<ViewId, ViewSnapshot>);

	/// Undoes all documents in the given list.
	/// Returns `true` if all undos succeeded.
	fn undo_documents(&mut self, doc_ids: &[DocumentId]) -> bool;

	/// Redoes all documents in the given list.
	/// Returns `true` if all redos succeeded.
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
	///
	/// This is primarily useful for testing and debugging.
	pub fn last_undo_group(&self) -> Option<&EditorUndoGroup> {
		self.undo_stack.last()
	}

	/// Returns a reference to the last redo group, if any.
	pub fn last_redo_group(&self) -> Option<&EditorUndoGroup> {
		self.redo_stack.last()
	}

	/// Pushes an undo group directly and clears the redo stack.
	///
	/// This is used for batch operations (like LSP workspace edits) that
	/// manage their own snapshot collection.
	pub fn push_group(&mut self, group: EditorUndoGroup) {
		trace!(
			docs = ?group.affected_docs,
			origin = ?group.origin,
			snapshots = group.view_snapshots.len(),
			undo_stack = self.undo_stack.len() + 1,
			"undo group pushed (direct)"
		);
		self.undo_stack.push(group);
		if !self.redo_stack.is_empty() {
			trace!(cleared = self.redo_stack.len(), "redo stack cleared");
		}
		self.redo_stack.clear();
	}

	/// Prepares an edit operation by capturing pre-edit state.
	///
	/// Call this before applying a transaction. The returned [`PreparedEdit`]
	/// should be passed to [`finalize_edit`] after the transaction is applied.
	///
	/// [`finalize_edit`]: Self::finalize_edit
	pub fn prepare_edit(
		&self,
		host: &impl UndoHost,
		buffer_id: ViewId,
		undo: UndoPolicy,
		origin: EditOrigin,
	) -> PreparedEdit {
		let doc_id = host.doc_id_for_buffer(buffer_id);
		let pre_views = host.collect_view_snapshots(doc_id);

		let start_new_group = !matches!(undo, UndoPolicy::NoUndo);

		PreparedEdit {
			affected_docs: vec![doc_id],
			pre_views,
			start_new_group,
			origin,
		}
	}

	/// Finalizes an edit operation by pushing an undo group if needed.
	///
	/// Call this after applying a transaction. If the transaction was applied
	/// successfully and should start a new undo group, this pushes the group
	/// and clears the redo stack.
	pub fn finalize_edit(&mut self, result: &CommitResult, prep: PreparedEdit) {
		#[cfg(test)]
		{
			self.finalize_calls += 1;
		}

		if result.applied && prep.start_new_group && result.undo_recorded {
			trace!(
				docs = ?prep.affected_docs,
				origin = ?prep.origin,
				snapshots = prep.pre_views.len(),
				undo_stack = self.undo_stack.len() + 1,
				"undo group pushed"
			);
			self.undo_stack.push(EditorUndoGroup {
				affected_docs: prep.affected_docs,
				view_snapshots: prep.pre_views,
				origin: prep.origin,
			});
			if !self.redo_stack.is_empty() {
				trace!(cleared = self.redo_stack.len(), "redo stack cleared");
			}
			self.redo_stack.clear();
		}
	}

	pub fn with_edit<H, F>(
		&mut self,
		host: &mut H,
		buffer_id: ViewId,
		undo: UndoPolicy,
		origin: EditOrigin,
		apply: F,
	) -> bool
	where
		H: UndoHost,
		F: FnOnce(&mut H) -> CommitResult,
	{
		let prep = self.prepare_edit(host, buffer_id, undo, origin);
		let result = apply(host);
		let applied = result.applied;
		self.finalize_edit(&result, prep);
		applied
	}

	pub fn with_undo_redo<H, F>(&mut self, host: &mut H, f: F)
	where
		H: UndoHost,
		F: FnOnce(&mut UndoManager, &mut H),
	{
		f(self, host);
	}

	/// Starts a deferred undo operation, capturing snapshots for later completion.
	pub fn start_remote_undo(&mut self, host: &mut impl UndoHost) -> Option<Vec<DocumentId>> {
		if self.pending_history.is_some() {
			return None;
		}
		if !host.guard_readonly() {
			return None;
		}

		let Some(group) = self.undo_stack.pop() else {
			trace!("undo: nothing to undo");
			host.notify_nothing_to_undo();
			return None;
		};

		let current_snapshots = host.capture_current_view_snapshots(&group.affected_docs);
		let remaining_docs = group.affected_docs.iter().copied().collect::<HashSet<_>>();
		let doc_ids = group.affected_docs.clone();

		self.pending_history = Some(PendingHistoryOp {
			kind: HistoryKind::Undo,
			group,
			current_snapshots,
			remaining_docs,
			restore_stack: true,
		});

		Some(doc_ids)
	}

	/// Starts a deferred redo operation, capturing snapshots for later completion.
	pub fn start_remote_redo(&mut self, host: &mut impl UndoHost) -> Option<Vec<DocumentId>> {
		if self.pending_history.is_some() {
			return None;
		}
		if !host.guard_readonly() {
			return None;
		}

		let Some(group) = self.redo_stack.pop() else {
			trace!("redo: nothing to redo");
			host.notify_nothing_to_redo();
			return None;
		};

		let current_snapshots = host.capture_current_view_snapshots(&group.affected_docs);
		let remaining_docs = group.affected_docs.iter().copied().collect::<HashSet<_>>();
		let doc_ids = group.affected_docs.clone();

		self.pending_history = Some(PendingHistoryOp {
			kind: HistoryKind::Redo,
			group,
			current_snapshots,
			remaining_docs,
			restore_stack: true,
		});

		Some(doc_ids)
	}

	/// Starts a blind deferred history operation for specific documents.
	pub fn start_blind_remote_history(
		&mut self,
		host: &mut impl UndoHost,
		kind: HistoryKind,
		doc_ids: Vec<DocumentId>,
	) -> bool {
		if self.pending_history.is_some() {
			return false;
		}
		if !host.guard_readonly() {
			return false;
		}

		let current_snapshots = host.capture_current_view_snapshots(&doc_ids);
		let remaining_docs = doc_ids.iter().copied().collect::<HashSet<_>>();

		self.pending_history = Some(PendingHistoryOp {
			kind,
			group: EditorUndoGroup {
				affected_docs: doc_ids,
				view_snapshots: HashMap::new(),
				origin: xeno_primitives::EditOrigin::Internal("blind-undo"),
			},
			current_snapshots,
			remaining_docs,
			restore_stack: false,
		});

		true
	}

	/// Notes that a deferred history delta was applied for the document.
	pub fn note_remote_history_delta(
		&mut self,
		host: &mut impl UndoHost,
		doc_id: DocumentId,
		kind: HistoryKind,
	) {
		let Some(mut pending) = self.pending_history.take() else {
			return;
		};

		if pending.kind != kind {
			self.pending_history = Some(pending);
			return;
		}

		if !pending.remaining_docs.remove(&doc_id) {
			self.pending_history = Some(pending);
			return;
		}

		if !pending.remaining_docs.is_empty() {
			self.pending_history = Some(pending);
			return;
		}

		match pending.kind {
			HistoryKind::Undo => {
				host.restore_view_snapshots(&pending.group.view_snapshots);
				self.redo_stack.push(EditorUndoGroup {
					affected_docs: pending.group.affected_docs,
					view_snapshots: pending.current_snapshots,
					origin: pending.group.origin,
				});
				host.notify_undo();
			}
			HistoryKind::Redo => {
				host.restore_view_snapshots(&pending.group.view_snapshots);
				self.undo_stack.push(EditorUndoGroup {
					affected_docs: pending.group.affected_docs,
					view_snapshots: pending.current_snapshots,
					origin: pending.group.origin,
				});
				host.notify_redo();
			}
		}
	}

	/// Cancels a pending deferred history operation, restoring stacks.
	pub fn cancel_pending_history(&mut self, host: &mut impl UndoHost, kind: HistoryKind) {
		let Some(pending) = self.pending_history.take() else {
			return;
		};

		if pending.kind != kind {
			self.pending_history = Some(pending);
			return;
		}

		match pending.kind {
			HistoryKind::Undo => {
				if pending.restore_stack {
					self.undo_stack.push(pending.group);
				}
				host.notify_nothing_to_undo();
			}
			HistoryKind::Redo => {
				if pending.restore_stack {
					self.redo_stack.push(pending.group);
				}
				host.notify_nothing_to_redo();
			}
		}
	}

	/// Cancels any pending deferred history operation.
	pub fn cancel_pending_history_any(&mut self, host: &mut impl UndoHost) {
		let Some(pending) = self.pending_history.take() else {
			return;
		};

		match pending.kind {
			HistoryKind::Undo => {
				if pending.restore_stack {
					self.undo_stack.push(pending.group);
				}
				host.notify_nothing_to_undo();
			}
			HistoryKind::Redo => {
				if pending.restore_stack {
					self.redo_stack.push(pending.group);
				}
				host.notify_nothing_to_redo();
			}
		}
	}

	/// Cancels any pending deferred history operation without notifications.
	pub fn cancel_pending_history_silent(&mut self) {
		let Some(pending) = self.pending_history.take() else {
			return;
		};

		if !pending.restore_stack {
			return;
		}

		match pending.kind {
			HistoryKind::Undo => {
				self.undo_stack.push(pending.group);
			}
			HistoryKind::Redo => {
				self.redo_stack.push(pending.group);
			}
		}
	}

	/// Undoes the last change, restoring view state for all affected buffers.
	///
	/// Returns `true` if undo was performed successfully.
	pub fn undo(&mut self, host: &mut impl UndoHost) -> bool {
		if self.pending_history.is_some() {
			return false;
		}
		if !host.guard_readonly() {
			return false;
		}

		let Some(group) = self.undo_stack.pop() else {
			trace!("undo: nothing to undo");
			host.notify_nothing_to_undo();
			return false;
		};

		trace!(
			docs = ?group.affected_docs,
			snapshots = group.view_snapshots.len(),
			origin = ?group.origin,
			undo_stack = self.undo_stack.len(),
			redo_stack = self.redo_stack.len(),
			"undo: popped group"
		);

		let current_snapshots = host.capture_current_view_snapshots(&group.affected_docs);
		let ok = host.undo_documents(&group.affected_docs);

		if ok {
			host.restore_view_snapshots(&group.view_snapshots);
			self.redo_stack.push(EditorUndoGroup {
				affected_docs: group.affected_docs,
				view_snapshots: current_snapshots,
				origin: group.origin,
			});
			trace!(
				redo_stack = self.redo_stack.len(),
				"undo: pushed to redo stack"
			);
			host.notify_undo();
			true
		} else {
			self.undo_stack.push(group);
			host.notify_nothing_to_undo();
			false
		}
	}

	/// Redoes the last undone change, restoring view state for all affected buffers.
	///
	/// Returns `true` if redo was performed successfully.
	pub fn redo(&mut self, host: &mut impl UndoHost) -> bool {
		if self.pending_history.is_some() {
			return false;
		}
		if !host.guard_readonly() {
			return false;
		}

		let Some(group) = self.redo_stack.pop() else {
			trace!("redo: nothing to redo");
			host.notify_nothing_to_redo();
			return false;
		};

		trace!(
			docs = ?group.affected_docs,
			snapshots = group.view_snapshots.len(),
			origin = ?group.origin,
			undo_stack = self.undo_stack.len(),
			redo_stack = self.redo_stack.len(),
			"redo: popped group"
		);

		let current_snapshots = host.capture_current_view_snapshots(&group.affected_docs);
		let ok = host.redo_documents(&group.affected_docs);

		if ok {
			host.restore_view_snapshots(&group.view_snapshots);
			self.undo_stack.push(EditorUndoGroup {
				affected_docs: group.affected_docs,
				view_snapshots: current_snapshots,
				origin: group.origin,
			});
			trace!(
				undo_stack = self.undo_stack.len(),
				"redo: pushed to undo stack"
			);
			host.notify_redo();
			true
		} else {
			self.redo_stack.push(group);
			host.notify_nothing_to_redo();
			false
		}
	}
}

#[cfg(test)]
mod tests;
