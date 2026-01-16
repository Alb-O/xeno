//! Editor-level undo/redo with view state restoration.
//!
//! Document history is managed at the document level (text content only).
//! Editor-level history captures view state (cursor, selection, scroll)
//! so that undo/redo restores the exact editing context.
//!
//! # Architecture
//!
//! - [`ViewSnapshot`]: Captures a single buffer's view state
//! - [`EditorUndoGroup`]: Groups affected documents with their view snapshots
//! - Document undo/redo: Restores text content
//! - Editor undo/redo: Calls document undo/redo then restores view snapshots

use std::collections::{HashMap, HashSet};

use tracing::warn;
use xeno_registry_notifications::keys;

use crate::buffer::{Buffer, BufferId, DocumentId};
use crate::impls::{Editor, EditorUndoGroup, ViewSnapshot};

impl Buffer {
	/// Creates a snapshot of this buffer's view state.
	pub fn snapshot_view(&self) -> ViewSnapshot {
		ViewSnapshot {
			cursor: self.cursor,
			selection: self.selection.clone(),
			scroll_line: self.scroll_line,
			scroll_segment: self.scroll_segment,
		}
	}

	/// Restores view state from a snapshot.
	pub fn restore_view(&mut self, snapshot: &ViewSnapshot) {
		self.cursor = snapshot.cursor;
		self.selection = snapshot.selection.clone();
		self.scroll_line = snapshot.scroll_line;
		self.scroll_segment = snapshot.scroll_segment;
		self.ensure_valid_selection();
	}
}

impl Editor {
	/// Collects view snapshots from all buffers sharing the same document.
	pub(crate) fn collect_view_snapshots(
		&self,
		doc_id: DocumentId,
	) -> HashMap<BufferId, ViewSnapshot> {
		self.buffers
			.buffers()
			.filter(|b| b.document_id() == doc_id)
			.map(|b| (b.id, b.snapshot_view()))
			.collect()
	}

	/// Restores view snapshots to buffers.
	///
	/// For buffers that have a snapshot, restores the exact view state.
	/// For buffers without a snapshot (e.g., created after the edit),
	/// just ensures the selection is valid.
	fn restore_view_snapshots(&mut self, snapshots: &HashMap<BufferId, ViewSnapshot>) {
		for buffer in self.buffers.buffers_mut() {
			if let Some(snapshot) = snapshots.get(&buffer.id) {
				buffer.restore_view(snapshot);
			} else {
				buffer.ensure_valid_selection();
			}
		}
	}

	/// Undoes the last change, restoring view state for all affected buffers.
	pub fn undo(&mut self) {
		if !self.guard_readonly() {
			return;
		}
		let Some(group) = self.undo_group_stack.pop() else {
			self.notify(keys::nothing_to_undo);
			return;
		};

		let current_snapshots = self.capture_current_view_snapshots(&group.affected_docs);

		let ok = self.undo_documents(&group.affected_docs);

		if ok {
			self.restore_view_snapshots(&group.view_snapshots);

			self.redo_group_stack.push(EditorUndoGroup {
				affected_docs: group.affected_docs,
				view_snapshots: current_snapshots,
				origin: group.origin,
			});
			self.notify(keys::undo);
		} else {
			self.undo_group_stack.push(group);
			self.notify(keys::nothing_to_undo);
		}
	}

	/// Redoes the last undone change, restoring view state for all affected buffers.
	pub fn redo(&mut self) {
		if !self.guard_readonly() {
			return;
		}
		let Some(group) = self.redo_group_stack.pop() else {
			self.notify(keys::nothing_to_redo);
			return;
		};

		let current_snapshots = self.capture_current_view_snapshots(&group.affected_docs);

		let ok = self.redo_documents(&group.affected_docs);

		if ok {
			self.restore_view_snapshots(&group.view_snapshots);

			self.undo_group_stack.push(EditorUndoGroup {
				affected_docs: group.affected_docs,
				view_snapshots: current_snapshots,
				origin: group.origin,
			});
			self.notify(keys::redo);
		} else {
			self.redo_group_stack.push(group);
			self.notify(keys::nothing_to_redo);
		}
	}

	/// Captures current view snapshots for all buffers viewing the given documents.
	fn capture_current_view_snapshots(
		&self,
		doc_ids: &[DocumentId],
	) -> HashMap<BufferId, ViewSnapshot> {
		let doc_set: HashSet<_> = doc_ids.iter().copied().collect();
		self.buffers
			.buffers()
			.filter(|b| doc_set.contains(&b.document_id()))
			.map(|b| (b.id, b.snapshot_view()))
			.collect()
	}

	/// Undoes a single document's last change.
	fn undo_document(&mut self, doc_id: DocumentId) -> bool {
		let buffer_id = self
			.buffers
			.buffers()
			.find(|b| b.document_id() == doc_id)
			.map(|b| b.id);

		let Some(buffer_id) = buffer_id else {
			warn!(doc_id = ?doc_id, "Undo: no buffer for document");
			return false;
		};

		let ok = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("buffer exists")
			.with_doc_mut(|doc| doc.undo(&self.config.language_loader));

		if ok {
			self.mark_buffer_dirty_for_full_sync(buffer_id);
		}
		ok
	}

	/// Redoes a single document's last undone change.
	fn redo_document(&mut self, doc_id: DocumentId) -> bool {
		let buffer_id = self
			.buffers
			.buffers()
			.find(|b| b.document_id() == doc_id)
			.map(|b| b.id);

		let Some(buffer_id) = buffer_id else {
			warn!(doc_id = ?doc_id, "Redo: no buffer for document");
			return false;
		};

		let ok = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("buffer exists")
			.with_doc_mut(|doc| doc.redo(&self.config.language_loader));

		if ok {
			self.mark_buffer_dirty_for_full_sync(buffer_id);
		}
		ok
	}

	/// Undoes all documents in the given list.
	fn undo_documents(&mut self, doc_ids: &[DocumentId]) -> bool {
		let mut ok = true;
		for &doc_id in doc_ids {
			ok &= self.undo_document(doc_id);
		}
		ok
	}

	/// Redoes all documents in the given list.
	fn redo_documents(&mut self, doc_ids: &[DocumentId]) -> bool {
		let mut ok = true;
		for &doc_id in doc_ids {
			ok &= self.redo_document(doc_id);
		}
		ok
	}
}
