//! Implementation of EditorCapabilities for Editor.

use ropey::RopeSlice;
use tome_base::Selection;
use tome_base::range::CharIdx;
use tome_manifest::editor_ctx::{
	BufferOpsAccess, CursorAccess, EditAccess, EditorCapabilities, MessageAccess, ModeAccess,
	SearchAccess, SelectionAccess, SelectionOpsAccess, TextAccess, ThemeAccess, UndoAccess,
};
use tome_manifest::{EditAction, Mode};

use crate::editor::Editor;

impl CursorAccess for Editor {
	fn cursor(&self) -> CharIdx {
		self.buffer().cursor
	}

	fn set_cursor(&mut self, pos: CharIdx) {
		self.buffer_mut().cursor = pos;
	}
}

impl SelectionAccess for Editor {
	fn selection(&self) -> &Selection {
		&self.buffer().selection
	}

	fn selection_mut(&mut self) -> &mut Selection {
		&mut self.buffer_mut().selection
	}

	fn set_selection(&mut self, sel: Selection) {
		self.buffer_mut().selection = sel;
	}
}

impl TextAccess for Editor {
	fn text(&self) -> RopeSlice<'_> {
		self.buffer().doc.slice(..)
	}
}

impl ModeAccess for Editor {
	fn mode(&self) -> Mode {
		self.buffer().input.mode()
	}

	fn set_mode(&mut self, mode: Mode) {
		self.buffer_mut().input.set_mode(mode);
	}
}

impl MessageAccess for Editor {
	fn notify(&mut self, type_id: &str, msg: &str) {
		self.notify(type_id, msg);
	}

	fn clear_message(&mut self) {
		self.message = None;
	}
}

impl SearchAccess for Editor {
	fn search_next(&mut self, add_selection: bool, extend: bool) -> bool {
		self.do_search_next(add_selection, extend)
	}

	fn search_prev(&mut self, add_selection: bool, extend: bool) -> bool {
		self.do_search_prev(add_selection, extend)
	}

	fn use_selection_as_pattern(&mut self) -> bool {
		self.do_use_selection_as_search()
	}

	fn pattern(&self) -> Option<&str> {
		self.buffer().input.last_search().map(|(p, _)| p)
	}

	fn set_pattern(&mut self, pattern: &str) {
		self.buffer_mut()
			.input
			.set_last_search(pattern.to_string(), false);
	}
}

impl UndoAccess for Editor {
	fn save_state(&mut self) {
		self.save_undo_state();
	}

	fn undo(&mut self) {
		self.undo();
	}

	fn redo(&mut self) {
		self.redo();
	}

	fn can_undo(&self) -> bool {
		!self.buffer().undo_stack.is_empty()
	}

	fn can_redo(&self) -> bool {
		!self.buffer().redo_stack.is_empty()
	}
}

impl EditAccess for Editor {
	fn execute_edit(&mut self, action: &EditAction, extend: bool) {
		self.do_execute_edit_action(action.clone(), extend);
	}
}

impl SelectionOpsAccess for Editor {
	fn split_lines(&mut self) -> bool {
		self.do_split_lines()
	}

	fn merge_selections(&mut self) {
		self.buffer_mut().selection.merge_overlaps_and_adjacent();
	}
}

impl ThemeAccess for Editor {
	fn set_theme(&mut self, name: &str) -> Result<(), tome_manifest::CommandError> {
		Editor::set_theme(self, name)
	}
}

impl BufferOpsAccess for Editor {
	fn split_horizontal(&mut self) {
		// Cannot split with buffer content when terminal is focused
		if self.is_terminal_focused() {
			return;
		}

		// Create a new buffer with the same content as current
		let current = self.buffer();
		let content: String = current.doc.slice(..).into();
		let path = current.path.clone();
		let new_id = self.open_buffer(content, path);
		Editor::split_horizontal(self, new_id);
	}

	fn split_vertical(&mut self) {
		// Cannot split with buffer content when terminal is focused
		if self.is_terminal_focused() {
			return;
		}

		// Create a new buffer with the same content as current
		let current = self.buffer();
		let content: String = current.doc.slice(..).into();
		let path = current.path.clone();
		let new_id = self.open_buffer(content, path);
		Editor::split_vertical(self, new_id);
	}

	fn split_terminal_horizontal(&mut self) {
		Editor::split_horizontal_terminal(self);
	}

	fn split_terminal_vertical(&mut self) {
		Editor::split_vertical_terminal(self);
	}

	fn buffer_next(&mut self) {
		self.focus_next_buffer();
	}

	fn buffer_prev(&mut self) {
		self.focus_prev_buffer();
	}

	fn close_buffer(&mut self) {
		self.close_current_buffer();
	}

	fn close_other_buffers(&mut self) {
		// Close all buffers except the current one
		let Some(current_id) = self.focused_buffer_id() else {
			return;
		};
		let ids: Vec<_> = self
			.buffer_ids()
			.into_iter()
			.filter(|&id| id != current_id)
			.collect();
		for id in ids {
			Editor::close_buffer(self, id);
		}
	}

	fn focus_left(&mut self) {
		// For now, just cycle to prev buffer (proper split navigation would need layout awareness)
		self.focus_prev_buffer();
	}

	fn focus_right(&mut self) {
		// For now, just cycle to next buffer (proper split navigation would need layout awareness)
		self.focus_next_buffer();
	}

	fn focus_up(&mut self) {
		self.focus_prev_buffer();
	}

	fn focus_down(&mut self) {
		self.focus_next_buffer();
	}
}

impl EditorCapabilities for Editor {
	fn search(&mut self) -> Option<&mut dyn SearchAccess> {
		Some(self)
	}

	fn undo(&mut self) -> Option<&mut dyn UndoAccess> {
		Some(self)
	}

	fn edit(&mut self) -> Option<&mut dyn EditAccess> {
		Some(self)
	}

	fn selection_ops(&mut self) -> Option<&mut dyn SelectionOpsAccess> {
		Some(self)
	}

	fn buffer_ops(&mut self) -> Option<&mut dyn BufferOpsAccess> {
		Some(self)
	}
}
