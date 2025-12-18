//! Implementation of EditorCapabilities for Editor.

use tome_core::ext::EditAction;
use tome_core::ext::editor_ctx::{
	CursorAccess, EditAccess, EditorCapabilities, MessageAccess, ModeAccess, ScratchAccess,
	SearchAccess, SelectionAccess, SelectionOpsAccess, TextAccess, UndoAccess,
};
use tome_core::{Mode, RopeSlice, Selection};

use crate::editor::Editor;

impl CursorAccess for Editor {
	fn cursor(&self) -> usize {
		self.cursor
	}

	fn set_cursor(&mut self, pos: usize) {
		self.cursor = pos;
	}
}

impl SelectionAccess for Editor {
	fn selection(&self) -> &Selection {
		&self.selection
	}

	fn set_selection(&mut self, sel: Selection) {
		self.selection = sel;
	}
}

impl TextAccess for Editor {
	fn text(&self) -> RopeSlice<'_> {
		self.doc.slice(..)
	}
}

impl ModeAccess for Editor {
	fn mode(&self) -> Mode {
		self.input.mode()
	}

	fn set_mode(&mut self, mode: Mode) {
		self.input.set_mode(mode);
	}
}

impl MessageAccess for Editor {
	fn show_message(&mut self, msg: &str) {
		self.show_message(msg);
	}

	fn show_error(&mut self, msg: &str) {
		self.show_error(msg);
	}

	fn clear_message(&mut self) {
		self.message = None;
	}
}

impl ScratchAccess for Editor {
	fn open(&mut self, focus: bool) {
		self.do_open_scratch(focus);
	}

	fn close(&mut self) {
		self.do_close_scratch();
	}

	fn toggle(&mut self) {
		self.do_toggle_scratch();
	}

	fn execute(&mut self) -> bool {
		self.do_execute_scratch()
	}

	fn is_open(&self) -> bool {
		self.scratch_open
	}

	fn is_focused(&self) -> bool {
		self.scratch_focused
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
		self.input.last_search().map(|(p, _)| p)
	}

	fn set_pattern(&mut self, pattern: &str) {
		self.input.set_last_search(pattern.to_string(), false);
	}
}

impl UndoAccess for Editor {
	fn save_state(&mut self) {
		self.save_undo_state();
	}

	fn undo(&mut self) -> bool {
		self.undo();
		false
	}

	fn redo(&mut self) -> bool {
		self.redo();
		false
	}

	fn can_undo(&self) -> bool {
		!self.undo_stack.is_empty()
	}

	fn can_redo(&self) -> bool {
		!self.redo_stack.is_empty()
	}
}

impl EditAccess for Editor {
	fn execute_edit(&mut self, action: &EditAction, extend: bool) -> bool {
		self.do_execute_edit_action(action.clone(), extend)
	}
}

impl SelectionOpsAccess for Editor {
	fn split_lines(&mut self) -> bool {
		self.do_split_lines()
	}

	fn merge_selections(&mut self) {
		// TODO: implement
	}

	fn duplicate_down(&mut self) {
		// TODO: implement
	}

	fn duplicate_up(&mut self) {
		// TODO: implement
	}
}

impl EditorCapabilities for Editor {
	fn scratch(&mut self) -> Option<&mut dyn ScratchAccess> {
		Some(self)
	}

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
}
