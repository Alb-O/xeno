//! Undo/redo history for buffers.

use tome_language::LanguageLoader;

use super::Buffer;
use crate::editor::types::HistoryEntry;

/// Result of an undo/redo operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryResult {
	/// Operation succeeded.
	Success,
	/// Nothing to undo.
	NothingToUndo,
	/// Nothing to redo.
	NothingToRedo,
}

impl Buffer {
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

	/// Saves current state to undo history.
	///
	/// Explicit calls reset any grouped insert session.
	pub fn save_undo_state(&mut self) {
		self.insert_undo_active = false;
		self.push_undo_snapshot();
	}

	/// Saves undo state for insert mode, grouping consecutive inserts.
	pub fn save_insert_undo_state(&mut self) {
		if self.insert_undo_active {
			return;
		}
		self.insert_undo_active = true;
		self.push_undo_snapshot();
	}

	/// Undoes the last change.
	///
	/// Returns the result of the operation. The caller is responsible for
	/// displaying any notifications to the user.
	pub fn undo(&mut self, language_loader: &LanguageLoader) -> HistoryResult {
		self.insert_undo_active = false;
		if let Some(entry) = self.undo_stack.pop() {
			self.redo_stack.push(HistoryEntry {
				doc: self.doc.clone(),
				selection: self.selection.clone(),
			});

			self.doc = entry.doc;
			self.selection = entry.selection;
			self.cursor = self.selection.primary().head;
			self.reparse_syntax(language_loader);
			HistoryResult::Success
		} else {
			HistoryResult::NothingToUndo
		}
	}

	/// Redoes the last undone change.
	///
	/// Returns the result of the operation. The caller is responsible for
	/// displaying any notifications to the user.
	pub fn redo(&mut self, language_loader: &LanguageLoader) -> HistoryResult {
		self.insert_undo_active = false;
		if let Some(entry) = self.redo_stack.pop() {
			self.undo_stack.push(HistoryEntry {
				doc: self.doc.clone(),
				selection: self.selection.clone(),
			});

			self.doc = entry.doc;
			self.selection = entry.selection;
			self.cursor = self.selection.primary().head;
			self.reparse_syntax(language_loader);
			HistoryResult::Success
		} else {
			HistoryResult::NothingToRedo
		}
	}
}
