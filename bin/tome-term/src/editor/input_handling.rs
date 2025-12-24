use tome_core::{Key, KeyCode, KeyResult, Mode, Selection, SpecialKey, movement};

use crate::editor::Editor;

impl Editor {
	pub fn handle_key(&mut self, key: termina::event::KeyEvent) -> bool {
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

		self.handle_key_active(key)
	}

	pub(crate) fn handle_key_active(&mut self, key: termina::event::KeyEvent) -> bool {
		use tome_core::ext::{HookContext, emit_hook};

		self.message = None;

		let old_mode = self.mode();
		let key: Key = key.into();

		if let Mode::Command { .. } = self.mode()
			&& self.completions.active
			&& let KeyCode::Special(SpecialKey::Tab) = key.code
		{
			let len = self.completions.items.len();
			if len > 0 {
				let new_idx = if key.modifiers.shift {
					match self.completions.selected_idx {
						Some(idx) => (idx + len - 1) % len,
						None => len - 1,
					}
				} else {
					match self.completions.selected_idx {
						Some(idx) => (idx + 1) % len,
						None => 0,
					}
				};
				self.completions.selected_idx = Some(new_idx);
				let item = self.completions.items[new_idx].clone();
				if let Mode::Command { prompt, .. } = self.input.mode() {
					self.input.set_mode(Mode::Command {
						prompt,
						input: item.insert_text,
					});
				}
				return false;
			}
		}

		let result = self.input.handle_key(key);

		if let Mode::Command { .. } = self.mode() {
			// Update completions for keys other than Tab/BackTab (which return false).
			self.update_completions();
		} else {
			self.completions.active = false;
		}

		match result {
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
					self.insert_undo_active = false;
				}
				false
			}
			KeyResult::InsertChar(c) => {
				self.insert_text(&c.to_string());
				false
			}
			KeyResult::ExecuteCommand(cmd) => self.execute_command_line(&cmd),
			KeyResult::ExecuteSearch { pattern, reverse } => {
				self.input.set_last_search(pattern.clone(), reverse);
				let result = if reverse {
					movement::find_prev(self.doc.slice(..), &pattern, self.cursor)
				} else {
					movement::find_next(self.doc.slice(..), &pattern, self.cursor + 1)
				};
				match result {
					Ok(Some(range)) => {
						self.cursor = range.head;
						self.selection = Selection::single(range.min(), range.max());
						self.show_message(format!("Found: {}", pattern));
					}
					Ok(None) => {
						self.show_message(format!("Pattern not found: {}", pattern));
					}
					Err(e) => {
						self.show_error(format!("Regex error: {}", e));
					}
				}
				false
			}
			KeyResult::SelectRegex { pattern } => {
				self.select_regex(&pattern);
				false
			}
			KeyResult::SplitRegex { pattern } => {
				self.split_regex(&pattern);
				false
			}
			KeyResult::KeepMatching { pattern } => {
				self.keep_matching(&pattern, false);
				false
			}
			KeyResult::KeepNotMatching { pattern } => {
				self.keep_matching(&pattern, true);
				false
			}
			KeyResult::PipeReplace { command } => {
				self.show_error(format!("Pipe (replace) not yet implemented: {}", command));
				false
			}
			KeyResult::PipeIgnore { command } => {
				self.show_error(format!("Pipe (ignore) not yet implemented: {}", command));
				false
			}
			KeyResult::InsertOutput { command } => {
				self.show_error(format!("Insert output not yet implemented: {}", command));
				false
			}
			KeyResult::AppendOutput { command } => {
				self.show_error(format!("Append output not yet implemented: {}", command));
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

	pub fn handle_mouse(&mut self, mouse: termina::event::MouseEvent) -> bool {
		let width = self.window_width.unwrap_or(80);
		let height = self.window_height.unwrap_or(24);
		let has_command_line = self.input.command_line().is_some();
		let message_height = if has_command_line { 1 } else { 0 };
		let main_height = height.saturating_sub(message_height + 1);
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

		self.handle_mouse_active(mouse)
	}

	pub(crate) fn handle_mouse_active(&mut self, mouse: termina::event::MouseEvent) -> bool {
		let result = self.input.handle_mouse(mouse.into());
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
		if let Some(doc_pos) = self.screen_to_doc_position(screen_row, screen_col) {
			if extend {
				let anchor = self.selection.primary().anchor;
				self.selection = Selection::single(anchor, doc_pos);
			} else {
				self.selection = Selection::point(doc_pos);
			}
		}
	}

	pub(crate) fn handle_mouse_drag(&mut self, screen_row: u16, screen_col: u16) {
		if let Some(doc_pos) = self.screen_to_doc_position(screen_row, screen_col) {
			let anchor = self.selection.primary().anchor;
			self.selection = Selection::single(anchor, doc_pos);
		}
	}
}
