//! Implementation of [`EditorCapabilities`] for [`Editor`].
//!
//! [`EditorCapabilities`]: evildoer_manifest::editor_ctx::EditorCapabilities

use evildoer_base::Selection;
use evildoer_base::range::CharIdx;
use evildoer_manifest::editor_ctx::{
	CommandQueueAccess, CursorAccess, EditAccess, EditorCapabilities, FileOpsAccess, FocusOps,
	JumpAccess, MacroAccess, MessageAccess, ModeAccess, PanelOps, SearchAccess, SelectionAccess,
	SplitOps, ThemeAccess, UndoAccess,
};
use evildoer_manifest::{EditAction, Mode, panel_kind_index};

use crate::buffer::BufferView;
use crate::editor::Editor;

fn panel_visible(editor: &Editor, name: &str) -> bool {
	let Some(kind) = panel_kind_index(name) else {
		return false;
	};
	let Some(panel_id) = editor.panels.find_by_kind(kind) else {
		return false;
	};
	editor.layout.contains_view(BufferView::Panel(panel_id))
}

impl CursorAccess for Editor {
	fn cursor(&self) -> CharIdx {
		self.buffer().cursor
	}

	fn cursor_line_col(&self) -> Option<(usize, usize)> {
		if !self.is_text_focused() {
			return None;
		}
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
		self.buffer_mut().input.set_mode(mode);
	}
}

impl MessageAccess for Editor {
	fn notify(&mut self, type_id: &str, msg: &str) {
		self.notify(type_id, msg);
	}

	fn clear_message(&mut self) {}
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
	fn set_theme(&mut self, name: &str) -> Result<(), evildoer_manifest::CommandError> {
		Editor::set_theme(self, name)
	}
}

impl SplitOps for Editor {
	fn split_horizontal(&mut self) {
		// Cannot split with buffer content when terminal is focused
		if self.is_terminal_focused() {
			return;
		}

		// Create a new buffer that shares the same document
		let new_id = self.clone_buffer_for_split();
		Editor::split_horizontal(self, new_id);
	}

	fn split_vertical(&mut self) {
		// Cannot split with buffer content when terminal is focused
		if self.is_terminal_focused() {
			return;
		}

		// Create a new buffer that shares the same document
		let new_id = self.clone_buffer_for_split();
		Editor::split_vertical(self, new_id);
	}

	fn close_split(&mut self) {
		self.close_current_view();
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
}

impl PanelOps for Editor {
	fn toggle_terminal(&mut self) {
		Editor::toggle_panel(self, "terminal");
	}

	fn toggle_debug_panel(&mut self) {
		Editor::toggle_panel(self, "debug");
	}

	fn toggle_panel(&mut self, name: &str) {
		Editor::toggle_panel(self, name);
	}

	fn open_panel(&mut self, name: &str) {
		if panel_visible(self, name) {
			return;
		}
		Editor::toggle_panel(self, name);
	}

	fn close_panel(&mut self, name: &str) {
		if !panel_visible(self, name) {
			return;
		}
		Editor::toggle_panel(self, name);
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

impl JumpAccess for Editor {
	fn jump_forward(&mut self) -> bool {
		if let Some(loc) = self.jump_list.jump_forward() {
			let buffer_id = loc.buffer_id;
			let cursor = loc.cursor;
			// Focus the buffer if different
			if self.focused_buffer_id() != Some(buffer_id) {
				self.focus_buffer(buffer_id);
			}
			// Set cursor position
			self.buffer_mut().cursor = cursor;
			true
		} else {
			false
		}
	}

	fn jump_backward(&mut self) -> bool {
		// Save current position before jumping back (if at end of list)
		if let Some(buffer_id) = self.focused_buffer_id() {
			let cursor = self.buffer().cursor;
			// Only save if we're at the end of the jump list
			self.jump_list
				.push(crate::editor::JumpLocation { buffer_id, cursor });
		}

		if let Some(loc) = self.jump_list.jump_backward() {
			let buffer_id = loc.buffer_id;
			let cursor = loc.cursor;
			// Focus the buffer if different
			if self.focused_buffer_id() != Some(buffer_id) {
				self.focus_buffer(buffer_id);
			}
			// Set cursor position
			self.buffer_mut().cursor = cursor;
			true
		} else {
			false
		}
	}

	fn save_jump(&mut self) {
		if let Some(buffer_id) = self.focused_buffer_id() {
			let cursor = self.buffer().cursor;
			self.jump_list
				.push(crate::editor::JumpLocation { buffer_id, cursor });
		}
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
		// Play last recorded macro
		// Actual playback would need to be handled by the input system
		// This is a placeholder - full implementation requires event loop integration
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

	fn panel_ops(&mut self) -> Option<&mut dyn PanelOps> {
		Some(self)
	}

	fn focus_ops(&mut self) -> Option<&mut dyn FocusOps> {
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
}
