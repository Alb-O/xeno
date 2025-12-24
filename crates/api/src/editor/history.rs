use crate::editor::Editor;
use crate::editor::types::HistoryEntry;

impl Editor {
	pub(crate) fn push_undo_snapshot(&mut self) {
		self.undo_stack.push(HistoryEntry {
			doc: self.doc.clone(),
			selection: self.selection.clone(),
		});
		self.redo_stack.clear();

		const MAX_UNDO: usize = 100;
		if self.undo_stack.len() > MAX_UNDO {
			self.undo_stack.remove(0);
		}
	}

	pub fn save_undo_state(&mut self) {
		// Explicit calls reset any grouped insert session.
		self.insert_undo_active = false;
		self.push_undo_snapshot();
	}

	pub(crate) fn save_insert_undo_state(&mut self) {
		if self.insert_undo_active {
			return;
		}
		self.insert_undo_active = true;
		self.push_undo_snapshot();
	}

	pub fn undo(&mut self) {
		self.insert_undo_active = false;
		if let Some(entry) = self.undo_stack.pop() {
			self.redo_stack.push(HistoryEntry {
				doc: self.doc.clone(),
				selection: self.selection.clone(),
			});

			self.doc = entry.doc;
			self.selection = entry.selection;
			self.show_message("Undo");
		} else {
			self.show_message("Nothing to undo");
		}
	}

	pub fn redo(&mut self) {
		self.insert_undo_active = false;
		if let Some(entry) = self.redo_stack.pop() {
			self.undo_stack.push(HistoryEntry {
				doc: self.doc.clone(),
				selection: self.selection.clone(),
			});

			self.doc = entry.doc;
			self.selection = entry.selection;
			self.show_message("Redo");
		} else {
			self.show_message("Nothing to redo");
		}
	}
}
