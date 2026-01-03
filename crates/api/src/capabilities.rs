//! Implementation of [`EditorCapabilities`] for [`Editor`].
//!
//! [`EditorCapabilities`]: xeno_core::editor_ctx::EditorCapabilities

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use xeno_base::range::CharIdx;
use xeno_base::{Mode, Selection};
use xeno_core::editor_ctx::{
	CommandQueueAccess, CursorAccess, EditAccess, EditorCapabilities, FileOpsAccess, FocusOps,
	JumpAccess, MacroAccess, ModeAccess, NotificationAccess, SearchAccess, SelectionAccess,
	SplitOps, ThemeAccess, UndoAccess, ViewportAccess,
};
use xeno_registry::EditAction;
use xeno_registry::commands::{CommandEditorOps, CommandError};
use xeno_registry_notifications::{Notification, keys};

use crate::editor::Editor;

impl CursorAccess for Editor {
	fn cursor(&self) -> CharIdx {
		self.buffer().cursor
	}

	fn cursor_line_col(&self) -> Option<(usize, usize)> {
		let buffer = self.buffer();
		Some((buffer.cursor_line(), buffer.cursor_col()))
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

impl ModeAccess for Editor {
	fn mode(&self) -> Mode {
		self.buffer().input.mode()
	}

	fn set_mode(&mut self, mode: Mode) {
		if matches!(mode, Mode::Insert) && self.buffer().is_readonly() {
			NotificationAccess::emit(self, keys::buffer_readonly.into());
			return;
		}
		self.buffer_mut().input.set_mode(mode);
	}
}

impl NotificationAccess for Editor {
	fn emit(&mut self, notification: Notification) {
		self.show_notification(notification);
	}

	fn clear_notifications(&mut self) {
		self.clear_all_notifications();
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
		self.buffer().undo_stack_len() > 0
	}

	fn can_redo(&self) -> bool {
		self.buffer().redo_stack_len() > 0
	}
}

impl EditAccess for Editor {
	fn execute_edit(&mut self, action: &EditAction, extend: bool) {
		self.do_execute_edit_action(action.clone(), extend);
	}
}

impl ThemeAccess for Editor {
	fn set_theme(&mut self, name: &str) -> Result<(), CommandError> {
		Editor::set_theme(self, name)
	}
}

impl CommandEditorOps for Editor {
	fn emit(&mut self, notification: Notification) {
		self.show_notification(notification);
	}

	fn clear_notifications(&mut self) {
		self.clear_all_notifications();
	}

	fn is_modified(&self) -> bool {
		FileOpsAccess::is_modified(self)
	}

	fn is_readonly(&self) -> bool {
		self.buffer().is_readonly()
	}

	fn set_readonly(&mut self, readonly: bool) {
		self.buffer().set_readonly(readonly);
	}

	fn save(&mut self) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + '_>> {
		FileOpsAccess::save(self)
	}

	fn save_as(
		&mut self,
		path: PathBuf,
	) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + '_>> {
		FileOpsAccess::save_as(self, path)
	}

	fn set_theme(&mut self, name: &str) -> Result<(), CommandError> {
		ThemeAccess::set_theme(self, name)
	}
}

impl SplitOps for Editor {
	fn split_horizontal(&mut self) {
		// Create a new buffer that shares the same document
		let new_id = self.clone_buffer_for_split();
		Editor::split_horizontal(self, new_id);
	}

	fn split_vertical(&mut self) {
		// Create a new buffer that shares the same document
		let new_id = self.clone_buffer_for_split();
		Editor::split_vertical(self, new_id);
	}

	fn close_split(&mut self) {
		self.close_current_buffer();
	}

	fn close_other_buffers(&mut self) {
		// Close all buffers except the current one
		let current_id = self.focused_view();
		let ids: Vec<_> = self
			.buffer_ids()
			.into_iter()
			.filter(|&id| id != current_id)
			.collect();
		for id in ids {
			Editor::close_buffer(self, id);
		}
	}
}

impl FocusOps for Editor {
	fn buffer_next(&mut self) {
		self.focus_next_buffer();
	}

	fn buffer_prev(&mut self) {
		self.focus_prev_buffer();
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

impl ViewportAccess for Editor {
	fn viewport_height(&self) -> usize {
		self.buffer().last_viewport_height
	}

	fn viewport_row_to_doc_position(&self, row: usize) -> Option<CharIdx> {
		let buffer = self.buffer();
		if buffer.last_viewport_height == 0 {
			return None;
		}
		buffer
			.screen_to_doc_position(row as u16, buffer.gutter_width())
			.map(|pos| pos as CharIdx)
	}
}

impl JumpAccess for Editor {
	fn jump_forward(&mut self) -> bool {
		if let Some(loc) = self.jump_list.jump_forward() {
			let buffer_id = loc.buffer_id;
			let cursor = loc.cursor;
			// Focus the buffer if different
			if self.focused_view() != buffer_id {
				self.focus_buffer(buffer_id);
			}
			self.buffer_mut().cursor = cursor;
			true
		} else {
			false
		}
	}

	fn jump_backward(&mut self) -> bool {
		let buffer_id = self.focused_view();
		let cursor = self.buffer().cursor;
		// Only save if we're at the end of the jump list
		self.jump_list
			.push(crate::editor::JumpLocation { buffer_id, cursor });

		if let Some(loc) = self.jump_list.jump_backward() {
			let buffer_id = loc.buffer_id;
			let cursor = loc.cursor;
			if self.focused_view() != buffer_id {
				self.focus_buffer(buffer_id);
			}
			self.buffer_mut().cursor = cursor;
			true
		} else {
			false
		}
	}

	fn save_jump(&mut self) {
		let buffer_id = self.focused_view();
		let cursor = self.buffer().cursor;
		self.jump_list
			.push(crate::editor::JumpLocation { buffer_id, cursor });
	}
}

impl MacroAccess for Editor {
	fn record(&mut self) {
		// Default to register 'q' if no register specified
		self.macro_state.start_recording('q');
	}

	fn stop_recording(&mut self) {
		self.macro_state.stop_recording();
	}

	fn play(&mut self) {
		// Actual playback requires event loop integration (placeholder).
	}

	fn is_recording(&self) -> bool {
		self.macro_state.is_recording()
	}
}

impl CommandQueueAccess for Editor {
	fn queue_command(&mut self, name: &'static str, args: Vec<String>) {
		self.command_queue.push(name, args);
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

	fn split_ops(&mut self) -> Option<&mut dyn SplitOps> {
		Some(self)
	}

	fn focus_ops(&mut self) -> Option<&mut dyn FocusOps> {
		Some(self)
	}

	fn viewport(&mut self) -> Option<&mut dyn ViewportAccess> {
		Some(self)
	}

	fn file_ops(&mut self) -> Option<&mut dyn FileOpsAccess> {
		Some(self)
	}

	fn jump_ops(&mut self) -> Option<&mut dyn JumpAccess> {
		Some(self)
	}

	fn macro_ops(&mut self) -> Option<&mut dyn MacroAccess> {
		Some(self)
	}

	fn command_queue(&mut self) -> Option<&mut dyn CommandQueueAccess> {
		Some(self)
	}

	fn is_readonly(&self) -> bool {
		self.buffer().is_readonly()
	}
}
