//! Undo/redo history for buffers.

use evildoer_language::LanguageLoader;

use super::Buffer;

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
	/// Saves current state to undo history.
	///
	/// Explicit calls reset any grouped insert session.
	pub fn save_undo_state(&self) {
		let selection = self.selection.clone();
		self.doc_mut().save_undo_state(&selection);
	}

	/// Saves undo state for insert mode, grouping consecutive inserts.
	pub fn save_insert_undo_state(&self) {
		let selection = self.selection.clone();
		self.doc_mut().save_insert_undo_state(&selection);
	}

	/// Undoes the last change.
	///
	/// Returns the result of the operation. The caller is responsible for
	/// displaying any notifications to the user.
	pub fn undo(&mut self, language_loader: &LanguageLoader) -> HistoryResult {
		let restored = self.doc_mut().undo(language_loader);
		if let Some(restored_selection) = restored {
			self.selection = restored_selection;
			self.cursor = self.selection.primary().head;
			self.ensure_valid_selection();
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
		self.ensure_valid_selection();
		let selection = self.selection.clone();
		let restored = self.doc_mut().redo(&selection, language_loader);
		if let Some(restored_selection) = restored {
			self.selection = restored_selection;
			self.cursor = self.selection.primary().head;
			self.ensure_valid_selection();
			HistoryResult::Success
		} else {
			HistoryResult::NothingToRedo
		}
	}
}
