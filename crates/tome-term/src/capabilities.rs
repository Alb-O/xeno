//! Implementation of EditorCapabilities for Editor.

use tome_core::ext::editor_ctx::{
	CursorAccess, EditAccess, EditorCapabilities, MessageAccess, ModeAccess, SearchAccess,
	SelectionAccess, SelectionOpsAccess, TextAccess, UndoAccess,
};
use tome_core::ext::{EditAction, EditorOps};
use tome_core::range::CharIdx;
use tome_core::{Mode, RopeSlice, Selection};

use crate::editor::Editor;

impl CursorAccess for Editor {
	fn cursor(&self) -> CharIdx {
		self.cursor
	}

	fn set_cursor(&mut self, pos: CharIdx) {
		self.cursor = pos;
	}
}

impl SelectionAccess for Editor {
	fn selection(&self) -> &Selection {
		&self.selection
	}

	fn selection_mut(&mut self) -> &mut Selection {
		&mut self.selection
	}

	fn set_selection(&mut self, sel: Selection) {
		self.selection = sel;
	}
}

impl EditorOps for Editor {
	fn path(&self) -> Option<&std::path::Path> {
		self.path.as_deref()
	}

	fn save(&mut self) -> Result<(), tome_core::ext::CommandError> {
		self.save()
	}

	fn save_as(&mut self, path: std::path::PathBuf) -> Result<(), tome_core::ext::CommandError> {
		self.save_as(path)
	}

	fn insert_text(&mut self, text: &str) {
		self.insert_text(text);
	}

	fn delete_selection(&mut self) {
		self.delete_selection();
	}

	fn set_modified(&mut self, modified: bool) {
		self.modified = modified;
	}

	fn is_modified(&self) -> bool {
		self.modified
	}

	fn set_theme(&mut self, theme_name: &str) -> Result<(), tome_core::ext::CommandError> {
		Editor::set_theme(self, theme_name)
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

	fn notify(&mut self, type_name: &str, msg: &str) {
		self.notify(type_name, msg);
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
		self.selection.merge_overlaps_and_adjacent();
	}

	fn duplicate_down(&mut self) {
		// TODO: implement
	}

	fn duplicate_up(&mut self) {
		// TODO: implement
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
}
