use tome_core::{Key, KeyCode, KeyResult, Mode, Selection, SpecialKey, movement};

use crate::editor::Editor;

impl Editor {
	pub fn handle_key(&mut self, key: termina::event::KeyEvent) -> bool {
		use termina::event::{KeyCode as TmKeyCode, Modifiers as TmModifiers};

		// Toggle terminal with Ctrl+t
		if matches!(key.code, TmKeyCode::Char('t')) && key.modifiers.contains(TmModifiers::CONTROL)
		{
			self.do_toggle_terminal();
			return false;
		}

		if self.plugins.plugins_open && self.plugins.plugins_focused {
			match key.code {
				TmKeyCode::Char('j') | TmKeyCode::Down => {
					let num_entries = self.plugins.entries.len();
					if num_entries > 0 {
						self.plugins.plugins_selected_idx =
							(self.plugins.plugins_selected_idx + 1) % num_entries;
					}
				}
				TmKeyCode::Char('k') | TmKeyCode::Up => {
					let num_entries = self.plugins.entries.len();
					if num_entries > 0 {
						self.plugins.plugins_selected_idx = self
							.plugins
							.plugins_selected_idx
							.checked_sub(1)
							.unwrap_or(num_entries - 1);
					}
				}
				TmKeyCode::Char(' ') | TmKeyCode::Enter => {
					let mut sorted_ids: Vec<_> = self.plugins.entries.keys().cloned().collect();
					sorted_ids.sort();
					if let Some(id) = sorted_ids.get(self.plugins.plugins_selected_idx) {
						let id = id.clone();
						let enabled = self.plugins.config.plugins.enabled.contains(&id);
						if enabled {
							let _ = self.plugin_command(&["disable", &id]);
						} else {
							let _ = self.plugin_command(&["enable", &id]);
						}
					}
				}
				TmKeyCode::Char('r') => {
					let mut sorted_ids: Vec<_> = self.plugins.entries.keys().cloned().collect();
					sorted_ids.sort();
					if let Some(id) = sorted_ids.get(self.plugins.plugins_selected_idx) {
						let id = id.clone();
						let _ = self.plugin_command(&["reload", &id]);
					}
				}
				TmKeyCode::Escape | TmKeyCode::Char('q') => {
					self.plugins.plugins_open = false;
					self.plugins.plugins_focused = false;
				}
				_ => {}
			}
			return false;
		}

		if self.handle_terminal_key(&key) {
			return false;
		}

		// Check plugin panels
		let mut panel_id_to_submit = None;
		let mut panel_handled = false;
		for panel in self.plugins.panels.values_mut() {
			if panel.open && panel.focused {
				let raw_ctrl_enter = matches!(
					key.code,
					TmKeyCode::Enter | TmKeyCode::Char('\n') | TmKeyCode::Char('j')
				) && key.modifiers.contains(TmModifiers::CONTROL);

				if raw_ctrl_enter {
					panel_id_to_submit = Some(panel.id);
				} else {
					match key.code {
						TmKeyCode::Char(c) => {
							panel.input.insert(panel.input_cursor, &c.to_string());
							panel.input_cursor += 1;
						}
						TmKeyCode::Backspace => {
							if panel.input_cursor > 0 {
								panel
									.input
									.remove(panel.input_cursor - 1..panel.input_cursor);
								panel.input_cursor -= 1;
							}
						}
						TmKeyCode::Enter => {
							panel.input.insert(panel.input_cursor, "\n");
							panel.input_cursor += 1;
						}
						TmKeyCode::Escape => {
							panel.focused = false;
						}
						_ => {}
					}
				}
				panel_handled = true;
				break;
			}
		}

		if let Some(panel_id) = panel_id_to_submit {
			self.submit_plugin_panel(panel_id);
			return false;
		}
		if panel_handled {
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
		if self.handle_terminal_mouse(&mouse) {
			return false;
		}

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
