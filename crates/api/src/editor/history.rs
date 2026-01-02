//! Editor-level undo/redo with multi-view selection sync.

use std::collections::HashMap;

use evildoer_base::Selection;

use crate::buffer::{BufferId, BufferView, DocumentId};
use crate::editor::Editor;

impl Editor {
	/// Collects selections from all buffers sharing the same document.
	fn collect_sibling_selections(&self, doc_id: DocumentId) -> HashMap<BufferId, Selection> {
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
					buffer.selection = selection.clone();
					buffer.cursor = buffer.selection.primary().head;
				}
				buffer.ensure_valid_selection();
			}
		}
	}

	/// Saves current state to undo history for all views of the focused document.
	pub fn save_undo_state(&mut self) {
		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			return;
		};
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
	}

	/// Saves undo state for insert mode, grouping consecutive inserts.
	pub(crate) fn save_insert_undo_state(&mut self) {
		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			return;
		};
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
			.save_insert_undo_state(selections);
	}

	/// Undoes the last change, restoring selections for all views of the document.
	pub fn undo(&mut self) {
		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			self.notify("warn", "Cannot undo in terminal");
			return;
		};
		let doc_id = self
			.buffers
			.get_buffer(buffer_id)
			.expect("focused buffer must exist")
			.document_id();
		let current = self.collect_sibling_selections(doc_id);

		let restored = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist")
			.doc_mut()
			.undo(current, &self.language_loader);

		let Some(selections) = restored else {
			self.notify("warn", "Nothing to undo");
			return;
		};
		self.restore_sibling_selections(doc_id, &selections);
		self.notify("info", "Undo");
	}

	/// Redoes the last undone change, restoring selections for all views of the document.
	pub fn redo(&mut self) {
		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			self.notify("warn", "Cannot redo in terminal");
			return;
		};
		let doc_id = self
			.buffers
			.get_buffer(buffer_id)
			.expect("focused buffer must exist")
			.document_id();
		let current = self.collect_sibling_selections(doc_id);

		let restored = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist")
			.doc_mut()
			.redo(current, &self.language_loader);

		let Some(selections) = restored else {
			self.notify("warn", "Nothing to redo");
			return;
		};
		self.restore_sibling_selections(doc_id, &selections);
		self.notify("info", "Redo");
	}
}
