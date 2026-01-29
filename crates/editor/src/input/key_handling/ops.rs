use xeno_primitives::Selection;

use super::types::ActionDispatch;
use crate::impls::Editor;
use crate::input::KeyResult;

impl Editor {
	/// Dispatches an action based on the key result.
	pub(crate) fn dispatch_action(&mut self, result: &KeyResult) -> ActionDispatch {
		use xeno_registry::find_action_by_id;

		match result {
			KeyResult::ActionById {
				id,
				count,
				extend,
				register,
			} => {
				let quit = if let Some(action) = find_action_by_id(*id) {
					self.invoke_action(action.name(), *count, *extend, *register, None)
						.is_quit()
				} else {
					self.show_notification(xeno_registry::notifications::keys::unknown_action(
						&id.to_string(),
					));
					false
				};
				ActionDispatch::Executed(quit)
			}
			KeyResult::ActionByIdWithChar {
				id,
				count,
				extend,
				register,
				char_arg,
			} => {
				let quit = if let Some(action) = find_action_by_id(*id) {
					self.invoke_action(action.name(), *count, *extend, *register, Some(*char_arg))
						.is_quit()
				} else {
					self.show_notification(xeno_registry::notifications::keys::unknown_action(
						&id.to_string(),
					));
					false
				};
				ActionDispatch::Executed(quit)
			}
			_ => ActionDispatch::NotAction,
		}
	}

	/// Updates LSP completion and signature help state after a key event.
	#[cfg(feature = "lsp")]
	pub(super) fn update_lsp_completion_state(
		&mut self,
		mode_change: Option<&xeno_primitives::Mode>,
		old_buffer_id: crate::buffer::ViewId,
		old_cursor: usize,
		old_version: u64,
		inserted_char: Option<char>,
	) {
		use xeno_lsp::CompletionTrigger;

		use crate::CompletionState;

		if let Some(new_mode) = mode_change
			&& !matches!(new_mode, xeno_primitives::Mode::Insert)
		{
			self.state.lsp.cancel_completion();
			self.cancel_signature_help();
			self.clear_lsp_menu();
		}

		let focus_changed = old_buffer_id != self.focused_view();
		let cursor_changed = old_cursor != self.buffer().cursor;
		let content_changed = old_version != self.buffer().version();

		let cursor = self.buffer().cursor;
		let menu_active = self
			.overlays()
			.get::<CompletionState>()
			.is_some_and(|s| s.active);
		let replace_start = self
			.overlays()
			.get::<CompletionState>()
			.map(|s| s.replace_start)
			.unwrap_or(0);

		if cursor < replace_start {
			self.state.lsp.cancel_completion();
			self.clear_lsp_menu();
		} else if menu_active && cursor_changed {
			self.state.frame.needs_redraw = true;
		}

		if focus_changed {
			self.state.lsp.cancel_completion();
			self.cancel_signature_help();
			self.clear_lsp_menu();
		} else if content_changed {
			self.cancel_signature_help();
			if self.buffer().mode() == xeno_primitives::Mode::Insert && !self.buffer().is_readonly()
			{
				// Refilter existing menu immediately (no LSP round-trip)
				if menu_active {
					self.refilter_completion();
				}
				// Also trigger LSP request for fresh results (debounced)
				self.trigger_lsp_completion(CompletionTrigger::Typing, inserted_char);
				if inserted_char == Some('(') {
					self.trigger_signature_help();
				}
			}
		} else if cursor_changed {
			self.cancel_signature_help();
		}
	}

	/// Handles a mouse click with view-local coordinates.
	pub(crate) fn handle_mouse_click_local(
		&mut self,
		local_row: u16,
		local_col: u16,
		extend: bool,
	) {
		let tab_width = self.tab_width();
		if let Some(doc_pos) = self
			.buffer()
			.screen_to_doc_position(local_row, local_col, tab_width)
		{
			let buffer = self.buffer_mut();
			if extend {
				let anchor = buffer.selection.primary().anchor;
				buffer.set_selection(Selection::single(anchor, doc_pos));
			} else {
				buffer.set_selection(Selection::point(doc_pos));
			}
			buffer.sync_cursor_to_selection();
			buffer.establish_goal_column();
			let view = self.focused_view();
			self.notify_overlay_event(crate::overlay::LayerEvent::CursorMoved { view });
		}
	}

	/// Handles mouse drag with view-local coordinates.
	pub(crate) fn handle_mouse_drag_local(&mut self, local_row: u16, local_col: u16) {
		let tab_width = self.tab_width();
		if let Some(doc_pos) = self
			.buffer()
			.screen_to_doc_position(local_row, local_col, tab_width)
		{
			let buffer = self.buffer_mut();
			let anchor = buffer.selection.primary().anchor;
			buffer.set_selection(Selection::single(anchor, doc_pos));
			buffer.sync_cursor_to_selection();
			let view = self.focused_view();
			self.notify_overlay_event(crate::overlay::LayerEvent::CursorMoved { view });
		}
	}
}
