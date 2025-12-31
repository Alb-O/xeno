//! Key event handling.
//!
//! Processing keyboard input and dispatching actions.

use evildoer_base::{Key, Selection};
use evildoer_input::KeyResult;
use evildoer_manifest::Mode;
use termina::event::KeyCode;

use super::conversions::convert_termina_key;
use crate::buffer::BufferView;
use crate::editor::Editor;

pub(crate) enum ActionDispatch {
	Executed(bool),
	NotAction,
}

impl Editor {
	pub(crate) fn dispatch_action(&mut self, result: &KeyResult) -> ActionDispatch {
		use evildoer_manifest::find_action_by_id;

		match result {
			KeyResult::ActionById {
				id,
				count,
				extend,
				register,
			} => {
				let quit = if let Some(action) = find_action_by_id(*id) {
					self.execute_action(action.name, *count, *extend, *register)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
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
					self.execute_action_with_char(
						action.name,
						*count,
						*extend,
						*register,
						*char_arg,
					)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				};
				ActionDispatch::Executed(quit)
			}
			_ => ActionDispatch::NotAction,
		}
	}

	pub async fn handle_key(&mut self, key: termina::event::KeyEvent) -> bool {
		// UI global bindings (panels, focus, etc.)
		if self.ui.handle_global_key(&key) {
			if self.ui.take_wants_redraw() {
				self.needs_redraw = true;
			}
			return false;
		}

		if self.ui.focused_panel_id().is_some() {
			let mut ui = std::mem::take(&mut self.ui);
			let _ = ui.handle_focused_key(self, key);
			if ui.take_wants_redraw() {
				self.needs_redraw = true;
			}
			self.ui = ui;
			return false;
		}

		// If a panel is focused, route input to it
		if let BufferView::Panel(panel_id) = self.focused_view() {
			let is_terminal = self.is_terminal_focused();

			// Ctrl+w enters window mode - use first buffer's input handler (terminal panels only)
			if is_terminal
				&& key.code == KeyCode::Char('w')
				&& key.modifiers.contains(termina::event::Modifiers::CONTROL)
			{
				if let Some(first_buffer_id) = self.layout.first_buffer()
					&& let Some(buffer) = self.buffers.get_buffer_mut(first_buffer_id)
				{
					buffer.input.set_mode(Mode::Window);
					self.needs_redraw = true;
				}
				return false;
			}

			// Escape releases focus back to the first text buffer
			if key.code == KeyCode::Escape {
				if let Some(first_buffer) = self.layout.first_buffer() {
					self.focus_buffer(first_buffer);
				}
				self.needs_redraw = true;
				return false;
			}

			// Check if we're in window mode (using first buffer's input handler, terminal panels only)
			if is_terminal && let Some(first_buffer_id) = self.layout.first_buffer() {
				let in_window_mode = self
					.buffers
					.get_buffer(first_buffer_id)
					.is_some_and(|b| matches!(b.input.mode(), Mode::Window));

				if in_window_mode {
					// Process window mode key through first buffer's input handler
					return self.handle_terminal_window_key(key, first_buffer_id).await;
				}
			}

			// Route all other keys to the panel
			if let Some(split_key) = convert_termina_key(&key) {
				let result = self.handle_panel_key(panel_id, split_key);
				if result.needs_redraw {
					self.needs_redraw = true;
				}
				if result.release_focus
					&& let Some(first_buffer) = self.layout.first_buffer()
				{
					self.focus_buffer(first_buffer);
				}
			}
			return false;
		}

		self.handle_key_active(key).await
	}

	pub(crate) async fn handle_key_active(&mut self, key: termina::event::KeyEvent) -> bool {
		use evildoer_manifest::{HookContext, HookEventData, emit_hook};

		let old_mode = self.mode();
		let key: Key = key.into();

		let result = self.buffer_mut().input.handle_key(key);

		if let ActionDispatch::Executed(quit) = self.dispatch_action(&result) {
			return quit;
		}

		match result {
			KeyResult::Pending { .. } => {
				self.needs_redraw = true;
				false
			}
			KeyResult::ModeChange(new_mode) => {
				let leaving_insert = !matches!(new_mode, Mode::Insert);
				if new_mode != old_mode {
					emit_hook(&HookContext::new(
						HookEventData::ModeChange {
							old_mode,
							new_mode: new_mode.clone(),
						},
						Some(&self.extensions),
					))
					.await;
				}
				if leaving_insert {
					self.buffer_mut().clear_insert_undo_active();
				}
				false
			}
			KeyResult::InsertChar(c) => {
				self.insert_text(&c.to_string());
				false
			}
			KeyResult::Consumed | KeyResult::Unhandled => false,
			KeyResult::Quit => true,
			KeyResult::MouseClick { row, col, extend } => {
				// Keyboard-triggered mouse events use screen coordinates relative to
				// the focused buffer's area. Translate them to view-local coordinates.
				let view_area = self.focused_view_area();
				let local_row = row.saturating_sub(view_area.y);
				let local_col = col.saturating_sub(view_area.x);
				self.handle_mouse_click_local(local_row, local_col, extend);
				false
			}
			KeyResult::MouseDrag { row, col } => {
				let view_area = self.focused_view_area();
				let local_row = row.saturating_sub(view_area.y);
				let local_col = col.saturating_sub(view_area.x);
				self.handle_mouse_drag_local(local_row, local_col);
				false
			}
			KeyResult::MouseScroll { direction, count } => {
				self.handle_mouse_scroll(direction, count);
				false
			}
			_ => unreachable!(),
		}
	}

	/// Handles window mode keys when a terminal is focused.
	async fn handle_terminal_window_key(
		&mut self,
		key: termina::event::KeyEvent,
		buffer_id: crate::buffer::BufferId,
	) -> bool {
		let key: Key = key.into();

		let result = {
			let Some(buffer) = self.buffers.get_buffer_mut(buffer_id) else {
				return false;
			};
			buffer.input.handle_key(key)
		};

		if let ActionDispatch::Executed(quit) = self.dispatch_action(&result) {
			return quit;
		}

		match result {
			KeyResult::Quit => true,
			KeyResult::ModeChange(_) | KeyResult::Consumed | KeyResult::Unhandled => {
				self.needs_redraw = true;
				false
			}
			_ => false,
		}
	}

	/// Handles a mouse click with view-local coordinates.
	pub(crate) fn handle_mouse_click_local(
		&mut self,
		local_row: u16,
		local_col: u16,
		extend: bool,
	) {
		if let Some(doc_pos) = self.buffer().screen_to_doc_position(local_row, local_col) {
			let buffer = self.buffer_mut();
			if extend {
				let anchor = buffer.selection.primary().anchor;
				buffer.selection = Selection::single(anchor, doc_pos);
			} else {
				buffer.selection = Selection::point(doc_pos);
			}
			buffer.cursor = buffer.selection.primary().head;
		}
	}

	/// Handles mouse drag with view-local coordinates.
	pub(crate) fn handle_mouse_drag_local(&mut self, local_row: u16, local_col: u16) {
		if let Some(doc_pos) = self.buffer().screen_to_doc_position(local_row, local_col) {
			let buffer = self.buffer_mut();
			let anchor = buffer.selection.primary().anchor;
			buffer.selection = Selection::single(anchor, doc_pos);
			buffer.cursor = buffer.selection.primary().head;
		}
	}
}
