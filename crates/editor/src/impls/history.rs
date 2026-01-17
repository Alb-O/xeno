//! Editor-level undo/redo with view state restoration.
//!
//! Document history is managed at the document level (text content only).
//! Editor-level history captures view state (cursor, selection, scroll)
//! so that undo/redo restores the exact editing context.
//!
//! # Architecture
//!
//! The undo system has two layers:
//!
//! - **Document layer**: Each document has its own undo stack storing text content.
//! - **Editor layer**: The [`UndoManager`] stores view state (cursor, selection, scroll)
//!   for all buffers affected by an edit.
//!
//! The [`UndoHost`] trait abstracts the Editor operations needed by UndoManager,
//! enabling cleaner separation of concerns.
//!
//! [`UndoManager`]: crate::types::UndoManager
//! [`UndoHost`]: crate::types::UndoHost

use std::collections::{HashMap, HashSet};

use tracing::warn;
use xeno_registry_notifications::keys;

use super::undo_host::EditorUndoHost;
use crate::buffer::{Buffer, DocumentId, ViewId};
use crate::impls::{Editor, UndoHost, ViewSnapshot};

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

impl UndoHost for Editor {
	fn guard_readonly(&mut self) -> bool {
		self.guard_readonly()
	}

	fn doc_id_for_buffer(&self, buffer_id: ViewId) -> DocumentId {
		self.core
			.buffers
			.get_buffer(buffer_id)
			.expect("buffer must exist")
			.document_id()
	}

	fn collect_view_snapshots(&self, doc_id: DocumentId) -> HashMap<ViewId, ViewSnapshot> {
		self.core
			.buffers
			.buffers()
			.filter(|b| b.document_id() == doc_id)
			.map(|b| (b.id, b.snapshot_view()))
			.collect()
	}

	fn capture_current_view_snapshots(
		&self,
		doc_ids: &[DocumentId],
	) -> HashMap<ViewId, ViewSnapshot> {
		let doc_set: HashSet<_> = doc_ids.iter().copied().collect();
		self.core
			.buffers
			.buffers()
			.filter(|b| doc_set.contains(&b.document_id()))
			.map(|b| (b.id, b.snapshot_view()))
			.collect()
	}

	fn restore_view_snapshots(&mut self, snapshots: &HashMap<ViewId, ViewSnapshot>) {
		for buffer in self.core.buffers.buffers_mut() {
			if let Some(snapshot) = snapshots.get(&buffer.id) {
				buffer.restore_view(snapshot);
			} else {
				buffer.ensure_valid_selection();
			}
		}
	}

	fn undo_documents(&mut self, doc_ids: &[DocumentId]) -> bool {
		let mut ok = true;
		for &doc_id in doc_ids {
			ok &= self.undo_document(doc_id);
		}
		ok
	}

	fn redo_documents(&mut self, doc_ids: &[DocumentId]) -> bool {
		let mut ok = true;
		for &doc_id in doc_ids {
			ok &= self.redo_document(doc_id);
		}
		ok
	}

	fn notify_undo(&mut self) {
		self.notify(keys::UNDO);
	}

	fn notify_redo(&mut self) {
		self.notify(keys::REDO);
	}

	fn notify_nothing_to_undo(&mut self) {
		self.notify(keys::NOTHING_TO_UNDO);
	}

	fn notify_nothing_to_redo(&mut self) {
		self.notify(keys::NOTHING_TO_REDO);
	}
}

impl Editor {
	/// Undoes the last change, restoring view state for all affected buffers.
	pub fn undo(&mut self) {
		let core = &mut self.core;
		let mut host = EditorUndoHost {
			buffers: &mut core.buffers,
			config: &self.config,
			frame: &mut self.frame,
			notifications: &mut self.notifications,
			#[cfg(feature = "lsp")]
			lsp: &mut self.lsp,
		};
		core.undo_manager.undo(&mut host);
	}

	/// Redoes the last undone change, restoring view state for all affected buffers.
	pub fn redo(&mut self) {
		let core = &mut self.core;
		let mut host = EditorUndoHost {
			buffers: &mut core.buffers,
			config: &self.config,
			frame: &mut self.frame,
			notifications: &mut self.notifications,
			#[cfg(feature = "lsp")]
			lsp: &mut self.lsp,
		};
		core.undo_manager.redo(&mut host);
	}

	/// Undoes a single document's last change.
	fn undo_document(&mut self, doc_id: DocumentId) -> bool {
		let buffer_id = self
			.core
			.buffers
			.buffers()
			.find(|b| b.document_id() == doc_id)
			.map(|b| b.id);

		let Some(buffer_id) = buffer_id else {
			warn!(doc_id = ?doc_id, "Undo: no buffer for document");
			return false;
		};

		let ok = self
			.core
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
			.core
			.buffers
			.buffers()
			.find(|b| b.document_id() == doc_id)
			.map(|b| b.id);

		let Some(buffer_id) = buffer_id else {
			warn!(doc_id = ?doc_id, "Redo: no buffer for document");
			return false;
		};

		let ok = self
			.core
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("buffer exists")
			.with_doc_mut(|doc| doc.redo(&self.config.language_loader));

		if ok {
			self.mark_buffer_dirty_for_full_sync(buffer_id);
		}
		ok
	}
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod history_tests;
