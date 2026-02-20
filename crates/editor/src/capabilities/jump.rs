use xeno_registry::actions::JumpAccess;

use crate::capabilities::provider::EditorCaps;
use crate::types::JumpLocation;

impl JumpAccess for EditorCaps<'_> {
	fn jump_forward(&mut self) -> bool {
		if let Some(loc) = self.ed.state.core.editor.workspace.jump_list.jump_forward() {
			let buffer_id = loc.buffer_id;
			let cursor = loc.cursor;
			if self.ed.focused_view() != buffer_id {
				self.ed.focus_buffer(buffer_id);
			}
			self.ed.buffer_mut().set_cursor(cursor);
			self.ed.snippet_session_on_cursor_moved(buffer_id);
			self.ed
				.state
				.runtime
				.effects
				.push_layer_event(crate::overlay::LayerEvent::CursorMoved { view: buffer_id });
			true
		} else {
			false
		}
	}

	fn jump_backward(&mut self) -> bool {
		let buffer_id = self.ed.focused_view();
		let cursor = self.ed.buffer().cursor;
		self.ed.state.core.editor.workspace.jump_list.push(JumpLocation { buffer_id, cursor });

		if let Some(loc) = self.ed.state.core.editor.workspace.jump_list.jump_backward() {
			let buffer_id = loc.buffer_id;
			let cursor = loc.cursor;
			if self.ed.focused_view() != buffer_id {
				self.ed.focus_buffer(buffer_id);
			}
			self.ed.buffer_mut().set_cursor(cursor);
			self.ed.snippet_session_on_cursor_moved(buffer_id);
			self.ed
				.state
				.runtime
				.effects
				.push_layer_event(crate::overlay::LayerEvent::CursorMoved { view: buffer_id });
			true
		} else {
			false
		}
	}

	fn save_jump(&mut self) {
		let buffer_id = self.ed.focused_view();
		let cursor = self.ed.buffer().cursor;
		self.ed.buffer_mut().clear_undo_group();
		self.ed.state.core.editor.workspace.jump_list.push(JumpLocation { buffer_id, cursor });
	}
}
