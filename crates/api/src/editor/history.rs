//! Editor-level undo/redo with multi-view selection sync.

use std::collections::{HashMap, HashSet};

use xeno_base::Selection;
use xeno_registry_notifications::keys;
use tracing::warn;

use crate::buffer::{BufferId, DocumentId};
use crate::editor::{Editor, EditorUndoEntry};

impl Editor {
	/// Collects selections from all buffers sharing the same document.
	pub(super) fn collect_sibling_selections(
		&self,
		doc_id: DocumentId,
	) -> HashMap<BufferId, Selection> {
		self.buffers
			.buffers()
			.filter(|b| b.document_id() == doc_id)
			.map(|b| (b.id, b.selection.clone()))
			.collect()
	}

	/// Restores saved selections to all buffers sharing the same document.
	fn restore_sibling_selections(
		&mut self,
		doc_id: DocumentId,
		selections: &HashMap<BufferId, Selection>,
	) {
		for buffer in self.buffers.buffers_mut() {
			if buffer.document_id() == doc_id {
				if let Some(selection) = selections.get(&buffer.id) {
					buffer.set_selection(selection.clone());
					buffer.sync_cursor_to_selection();
				}
				buffer.ensure_valid_selection();
			}
		}
	}

	/// Saves current state to undo history for all views of the focused document.
	pub fn save_undo_state(&mut self) {
		let buffer_id = self.focused_view();
		let doc_id = self
			.buffers
			.get_buffer(buffer_id)
			.expect("focused buffer must exist")
			.document_id();
		let selections = self.collect_sibling_selections(doc_id);
		self.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist")
			.doc_mut()
			.save_undo_state(selections);
		self.undo_group_stack
			.push(EditorUndoEntry::Single { buffer_id });
		self.redo_group_stack.clear();
	}

	/// Saves undo state for insert mode, grouping consecutive inserts.
	pub(crate) fn save_insert_undo_state(&mut self) {
		let buffer_id = self.focused_view();
		let doc_id = self
			.buffers
			.get_buffer(buffer_id)
			.expect("focused buffer must exist")
			.document_id();
		let selections = self.collect_sibling_selections(doc_id);
		let created = self.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist")
			.doc_mut()
			.save_insert_undo_state(selections);
		if created {
			self.undo_group_stack
				.push(EditorUndoEntry::Single { buffer_id });
			self.redo_group_stack.clear();
		}
	}

	/// Undoes the last change, restoring selections for all views of the document.
	pub fn undo(&mut self) {
		if !self.guard_readonly() {
			return;
		}
		let Some(entry) = self.undo_group_stack.pop() else {
			self.notify(keys::nothing_to_undo);
			return;
		};

		let ok = match &entry {
			EditorUndoEntry::Single { buffer_id } => self.undo_buffer(*buffer_id),
			EditorUndoEntry::Group { buffers } => self.undo_group(buffers),
		};

		if ok {
			self.redo_group_stack.push(entry);
			self.notify(keys::undo);
		} else {
			self.undo_group_stack.push(entry);
			self.notify(keys::nothing_to_undo);
		}
	}

	/// Redoes the last undone change, restoring selections for all views of the document.
	pub fn redo(&mut self) {
		if !self.guard_readonly() {
			return;
		}
		let Some(entry) = self.redo_group_stack.pop() else {
			self.notify(keys::nothing_to_redo);
			return;
		};

		let ok = match &entry {
			EditorUndoEntry::Single { buffer_id } => self.redo_buffer(*buffer_id),
			EditorUndoEntry::Group { buffers } => self.redo_group(buffers),
		};

		if ok {
			self.undo_group_stack.push(entry);
			self.notify(keys::redo);
		} else {
			self.redo_group_stack.push(entry);
			self.notify(keys::nothing_to_redo);
		}
	}

	fn undo_buffer(&mut self, buffer_id: BufferId) -> bool {
		let Some(buffer) = self.buffers.get_buffer(buffer_id) else {
			warn!(buffer_id = ?buffer_id, "Undo buffer missing");
			return false;
		};
		let doc_id = buffer.document_id();
		let current = self.collect_sibling_selections(doc_id);

		let restored = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("buffer exists")
			.doc_mut()
			.undo(current, &self.config.language_loader);

		let Some(selections) = restored else {
			return false;
		};

		self.mark_buffer_dirty_for_full_sync(buffer_id);
		self.restore_sibling_selections(doc_id, &selections);
		true
	}

	fn redo_buffer(&mut self, buffer_id: BufferId) -> bool {
		let Some(buffer) = self.buffers.get_buffer(buffer_id) else {
			warn!(buffer_id = ?buffer_id, "Redo buffer missing");
			return false;
		};
		let doc_id = buffer.document_id();
		let current = self.collect_sibling_selections(doc_id);

		let restored = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("buffer exists")
			.doc_mut()
			.redo(current, &self.config.language_loader);

		let Some(selections) = restored else {
			return false;
		};

		self.mark_buffer_dirty_for_full_sync(buffer_id);
		self.restore_sibling_selections(doc_id, &selections);
		true
	}

	fn undo_group(&mut self, buffers: &[BufferId]) -> bool {
		let mut seen = HashSet::new();
		let mut doc_ids = Vec::new();
		for &buffer_id in buffers.iter().rev() {
			let Some(buffer) = self.buffers.get_buffer(buffer_id) else {
				warn!(buffer_id = ?buffer_id, "Undo group buffer missing");
				continue;
			};
			let doc_id = buffer.document_id();
			if seen.insert(doc_id) {
				doc_ids.push(doc_id);
			}
		}

		let mut ok = true;
		for doc_id in doc_ids {
			let buffer_id = self
				.buffers
				.buffers()
				.find(|b| b.document_id() == doc_id)
				.map(|b| b.id);
			let Some(buffer_id) = buffer_id else {
				warn!(doc_id = ?doc_id, "Undo group missing document buffer");
				ok = false;
				continue;
			};
			ok &= self.undo_buffer(buffer_id);
		}
		ok
	}

	fn redo_group(&mut self, buffers: &[BufferId]) -> bool {
		let mut seen = HashSet::new();
		let mut doc_ids = Vec::new();
		for &buffer_id in buffers.iter().rev() {
			let Some(buffer) = self.buffers.get_buffer(buffer_id) else {
				warn!(buffer_id = ?buffer_id, "Redo group buffer missing");
				continue;
			};
			let doc_id = buffer.document_id();
			if seen.insert(doc_id) {
				doc_ids.push(doc_id);
			}
		}

		let mut ok = true;
		for doc_id in doc_ids {
			let buffer_id = self
				.buffers
				.buffers()
				.find(|b| b.document_id() == doc_id)
				.map(|b| b.id);
			let Some(buffer_id) = buffer_id else {
				warn!(doc_id = ?doc_id, "Redo group missing document buffer");
				ok = false;
				continue;
			};
			ok &= self.redo_buffer(buffer_id);
		}
		ok
	}
}
