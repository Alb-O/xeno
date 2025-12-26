use termina::event::{KeyCode, Modifiers};
use tome_base::{Key, Selection};
use tome_input::KeyResult;
use tome_manifest::{Mode, SplitBuffer, SplitKey, SplitKeyCode, SplitModifiers};

use crate::buffer::BufferView;
use crate::editor::Editor;

impl Editor {
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

		// If a terminal is focused, route input to it
		// Exception: Ctrl+w enters window mode, Escape releases focus
		if let BufferView::Terminal(terminal_id) = self.focused_view() {
			// Ctrl+w enters window mode - use first buffer's input handler
			if key.code == KeyCode::Char('w') && key.modifiers.contains(Modifiers::CONTROL) {
				if let Some(first_buffer_id) = self.layout.first_buffer() {
					if let Some(buffer) = self.buffers.get_mut(&first_buffer_id) {
						buffer.input.set_mode(Mode::Window);
						self.needs_redraw = true;
					}
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

			// Check if we're in window mode (using first buffer's input handler)
			if let Some(first_buffer_id) = self.layout.first_buffer() {
				let in_window_mode = self
					.buffers
					.get(&first_buffer_id)
					.is_some_and(|b| matches!(b.input.mode(), Mode::Window));

				if in_window_mode {
					// Process window mode key through first buffer's input handler
					return self.handle_terminal_window_key(key, first_buffer_id).await;
				}
			}

			// Route all other keys to the terminal
			if let Some(split_key) = convert_termina_key(&key) {
				if let Some(terminal) = self.terminals.get_mut(&terminal_id) {
					let result = terminal.handle_key(split_key);
					if result.needs_redraw {
						self.needs_redraw = true;
					}
				}
			}
			return false;
		}

		self.handle_key_active(key).await
	}

	pub(crate) async fn handle_key_active(&mut self, key: termina::event::KeyEvent) -> bool {
		use tome_manifest::{HookContext, emit_hook, find_action_by_id};

		self.message = None;

		let old_mode = self.mode();
		let key: Key = key.into();

		let result = self.buffer_mut().input.handle_key(key);

		match result {
			// Typed ActionId dispatch (preferred path)
			KeyResult::ActionById {
				id,
				count,
				extend,
				register,
			} => {
				if let Some(action) = find_action_by_id(id) {
					self.execute_action(action.name, count, extend, register)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				}
			}
			KeyResult::ActionByIdWithChar {
				id,
				count,
				extend,
				register,
				char_arg,
			} => {
				if let Some(action) = find_action_by_id(id) {
					self.execute_action_with_char(action.name, count, extend, register, char_arg)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				}
			}
			// String-based dispatch (backward compatibility)
			KeyResult::Action {
				name,
				count,
				extend,
				register,
			} => self.execute_action(name, count, extend, register),
			KeyResult::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			} => self.execute_action_with_char(name, count, extend, register, char_arg),
			KeyResult::ModeChange(new_mode) => {
				let is_normal = matches!(new_mode, Mode::Normal);
				let leaving_insert = !matches!(new_mode, Mode::Insert);
				if new_mode != old_mode {
					emit_hook(&HookContext::ModeChange {
						old_mode,
						new_mode: new_mode.clone(),
					});
				}
				if is_normal {
					self.message = None;
				}
				if leaving_insert {
					self.buffer_mut().insert_undo_active = false;
				}
				false
			}
			KeyResult::InsertChar(c) => {
				self.insert_text(&c.to_string());
				false
			}
			KeyResult::Consumed => false,
			KeyResult::Unhandled => false,
			KeyResult::Quit => true,
			KeyResult::MouseClick { row, col, extend } => {
				self.handle_mouse_click(row, col, extend);
				false
			}
			KeyResult::MouseDrag { row, col } => {
				self.handle_mouse_drag(row, col);
				false
			}
			KeyResult::MouseScroll { direction, count } => {
				self.handle_mouse_scroll(direction, count);
				false
			}
		}
	}

	/// Handles window mode keys when a terminal is focused.
	///
	/// Uses the specified buffer's input handler for window mode processing.
	async fn handle_terminal_window_key(
		&mut self,
		key: termina::event::KeyEvent,
		buffer_id: crate::buffer::BufferId,
	) -> bool {
		use tome_manifest::find_action_by_id;

		let key: Key = key.into();

		// Get the result from the buffer's input handler
		let result = {
			let Some(buffer) = self.buffers.get_mut(&buffer_id) else {
				return false;
			};
			buffer.input.handle_key(key)
		};

		match result {
			KeyResult::ActionById {
				id,
				count,
				extend,
				register,
			} => {
				if let Some(action) = find_action_by_id(id) {
					self.execute_action(action.name, count, extend, register)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				}
			}
			KeyResult::ActionByIdWithChar {
				id,
				count,
				extend,
				register,
				char_arg,
			} => {
				if let Some(action) = find_action_by_id(id) {
					self.execute_action_with_char(action.name, count, extend, register, char_arg)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				}
			}
			KeyResult::Action {
				name,
				count,
				extend,
				register,
			} => self.execute_action(name, count, extend, register),
			KeyResult::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			} => self.execute_action_with_char(name, count, extend, register, char_arg),
			KeyResult::ModeChange(_) | KeyResult::Consumed | KeyResult::Unhandled => {
				self.needs_redraw = true;
				false
			}
			KeyResult::Quit => true,
			// These shouldn't happen in window mode but handle gracefully
			KeyResult::InsertChar(_)
			| KeyResult::MouseClick { .. }
			| KeyResult::MouseDrag { .. }
			| KeyResult::MouseScroll { .. } => false,
		}
	}

	pub async fn handle_mouse(&mut self, mouse: termina::event::MouseEvent) -> bool {
		let width = self.window_width.unwrap_or(80);
		let height = self.window_height.unwrap_or(24);
		// Main area excludes status line (1 row)
		let main_height = height.saturating_sub(1);
		let main_area = ratatui::layout::Rect {
			x: 0,
			y: 0,
			width,
			height: main_height,
		};

		let mut ui = std::mem::take(&mut self.ui);
		let layout = ui.compute_layout(main_area);

		if ui.handle_mouse(self, mouse, &layout) {
			if ui.take_wants_redraw() {
				self.needs_redraw = true;
			}
			self.ui = ui;
			return false;
		}
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;

		self.handle_mouse_active(mouse).await
	}

	pub(crate) async fn handle_mouse_active(&mut self, mouse: termina::event::MouseEvent) -> bool {
		// Terminal views don't handle mouse events through the input system
		if self.is_terminal_focused() {
			return false;
		}

		let result = self.buffer_mut().input.handle_mouse(mouse.into());
		match result {
			KeyResult::MouseClick { row, col, extend } => {
				self.handle_mouse_click(row, col, extend);
				false
			}
			KeyResult::MouseDrag { row, col } => {
				self.handle_mouse_drag(row, col);
				false
			}
			KeyResult::MouseScroll { direction, count } => {
				self.handle_mouse_scroll(direction, count);
				false
			}
			_ => false,
		}
	}

	pub(crate) fn handle_mouse_click(&mut self, screen_row: u16, screen_col: u16, extend: bool) {
		// Terminal views don't support mouse click positioning
		if self.is_terminal_focused() {
			return;
		}

		if let Some(doc_pos) = self.buffer().screen_to_doc_position(screen_row, screen_col) {
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

	pub(crate) fn handle_mouse_drag(&mut self, screen_row: u16, screen_col: u16) {
		// Terminal views don't support mouse drag positioning
		if self.is_terminal_focused() {
			return;
		}

		if let Some(doc_pos) = self.buffer().screen_to_doc_position(screen_row, screen_col) {
			let buffer = self.buffer_mut();
			let anchor = buffer.selection.primary().anchor;
			buffer.selection = Selection::single(anchor, doc_pos);
			buffer.cursor = buffer.selection.primary().head;
		}
	}
}

/// Converts a termina KeyEvent to a SplitKey for terminal input.
fn convert_termina_key(key: &termina::event::KeyEvent) -> Option<SplitKey> {
	let code = match key.code {
		KeyCode::Char(c) => SplitKeyCode::Char(c),
		KeyCode::Enter => SplitKeyCode::Enter,
		KeyCode::Escape => SplitKeyCode::Escape,
		KeyCode::Backspace => SplitKeyCode::Backspace,
		KeyCode::Tab => SplitKeyCode::Tab,
		KeyCode::Up => SplitKeyCode::Up,
		KeyCode::Down => SplitKeyCode::Down,
		KeyCode::Left => SplitKeyCode::Left,
		KeyCode::Right => SplitKeyCode::Right,
		KeyCode::Home => SplitKeyCode::Home,
		KeyCode::End => SplitKeyCode::End,
		KeyCode::PageUp => SplitKeyCode::PageUp,
		KeyCode::PageDown => SplitKeyCode::PageDown,
		KeyCode::Delete => SplitKeyCode::Delete,
		KeyCode::Insert => SplitKeyCode::Insert,
		_ => return None,
	};

	let mut modifiers = SplitModifiers::NONE;
	if key.modifiers.contains(Modifiers::CONTROL) {
		modifiers = modifiers.union(SplitModifiers::CTRL);
	}
	if key.modifiers.contains(Modifiers::ALT) {
		modifiers = modifiers.union(SplitModifiers::ALT);
	}
	if key.modifiers.contains(Modifiers::SHIFT) {
		modifiers = modifiers.union(SplitModifiers::SHIFT);
	}

	Some(SplitKey::new(code, modifiers))
}
