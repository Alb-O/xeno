//! Editor-level undo/redo operations.
//!
//! These methods delegate to Buffer and handle user notifications.

use crate::buffer::{BufferView, HistoryResult};
use crate::editor::Editor;

impl Editor {
	/// Saves current state to undo history.
	///
	/// Delegates to Buffer's save_undo_state.
	pub fn save_undo_state(&mut self) {
		if self.is_text_focused() {
			self.buffer_mut().save_undo_state();
		}
	}

	/// Saves undo state for insert mode, grouping consecutive inserts.
	///
	/// Delegates to Buffer's save_insert_undo_state.
	pub(crate) fn save_insert_undo_state(&mut self) {
		if self.is_text_focused() {
			self.buffer_mut().save_insert_undo_state();
		}
	}

	/// Undoes the last change and notifies the user.
	pub fn undo(&mut self) {
		let BufferView::Text(buffer_id) = self.focused_view else {
			self.notify("warn", "Cannot undo in terminal");
			return;
		};

		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_mut(&buffer_id)
			.expect("focused buffer must exist");
		let result = buffer.undo(&self.language_loader);
		match result {
			HistoryResult::Success => self.notify("info", "Undo"),
			HistoryResult::NothingToUndo => self.notify("warn", "Nothing to undo"),
			HistoryResult::NothingToRedo => unreachable!(),
		}
	}

	/// Redoes the last undone change and notifies the user.
	pub fn redo(&mut self) {
		let BufferView::Text(buffer_id) = self.focused_view else {
			self.notify("warn", "Cannot redo in terminal");
			return;
		};

		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_mut(&buffer_id)
			.expect("focused buffer must exist");
		let result = buffer.redo(&self.language_loader);
		match result {
			HistoryResult::Success => self.notify("info", "Redo"),
			HistoryResult::NothingToRedo => self.notify("warn", "Nothing to redo"),
			HistoryResult::NothingToUndo => unreachable!(),
		}
	}
}
