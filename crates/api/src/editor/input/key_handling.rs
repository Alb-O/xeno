//! Key event handling.
//!
//! Processing keyboard input and dispatching actions.

use termina::event::KeyCode;
use xeno_base::{Key, Mode, Selection};
use xeno_input::KeyResult;

use crate::editor::{Editor, FocusTarget};
use crate::window::Window;

/// Result of attempting to dispatch an action from a key result.
pub(crate) enum ActionDispatch {
	/// Action was executed; bool indicates quit request.
	Executed(bool),
	/// Key result was not an action.
	NotAction,
}

impl Editor {
	/// Dispatches an action based on the key result.
	pub(crate) fn dispatch_action(&mut self, result: &KeyResult) -> ActionDispatch {
		use xeno_core::find_action_by_id;

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
					self.show_notification(
						xeno_registry_notifications::keys::unknown_action::call(&id.to_string()),
					);
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
					self.show_notification(
						xeno_registry_notifications::keys::unknown_action::call(&id.to_string()),
					);
					false
				};
				ActionDispatch::Executed(quit)
			}
			_ => ActionDispatch::NotAction,
		}
	}

	/// Processes a key event, routing to menus, UI, or input state machine.
	pub async fn handle_key(&mut self, key: termina::event::KeyEvent) -> bool {
		// Handle menu bar when active
		if self.menu.is_active() {
			self.handle_menu_key(&key);
			return false;
		}

		if key.code == KeyCode::Char('m') && key.modifiers.contains(termina::event::Modifiers::ALT)
		{
			self.menu.activate();
			self.needs_redraw = true;
			return false;
		}

		// UI global bindings (panels, focus, etc.)
		if self.ui.handle_global_key(&key) {
			if self.ui.take_wants_redraw() {
				self.needs_redraw = true;
			}
			self.sync_focus_from_ui();
			return false;
		}

		if self.ui.focused_panel_id().is_some() {
			let mut ui = std::mem::take(&mut self.ui);
			let _ = ui.handle_focused_key(self, key);
			if ui.take_wants_redraw() {
				self.needs_redraw = true;
			}
			self.ui = ui;
			self.sync_focus_from_ui();
			return false;
		}

		self.handle_key_active(key).await
	}

	/// Handles a key event when in active editing mode.
	pub(crate) async fn handle_key_active(&mut self, key: termina::event::KeyEvent) -> bool {
		use xeno_registry::{HookContext, HookEventData, emit as emit_hook};

		let old_mode = self.mode();

		if self.palette_is_open() && key.code == KeyCode::Enter {
			self.execute_palette();
			self.needs_redraw = true;
			return false;
		}

		if self.handle_floating_escape(&key) {
			return false;
		}
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
				if !self.guard_readonly() {
					return false;
				}
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

	/// Handles a mouse click with view-local coordinates.
	pub(crate) fn handle_mouse_click_local(
		&mut self,
		local_row: u16,
		local_col: u16,
		extend: bool,
	) {
		let tab_width = self.tab_width();
		if let Some(doc_pos) = self.buffer().screen_to_doc_position(local_row, local_col, tab_width)
		{
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
		let tab_width = self.tab_width();
		if let Some(doc_pos) = self.buffer().screen_to_doc_position(local_row, local_col, tab_width)
		{
			let buffer = self.buffer_mut();
			let anchor = buffer.selection.primary().anchor;
			buffer.selection = Selection::single(anchor, doc_pos);
			buffer.cursor = buffer.selection.primary().head;
		}
	}

	/// Handles key input when the menu bar is active.
	fn handle_menu_key(&mut self, key: &termina::event::KeyEvent) {
		match key.code {
			KeyCode::Escape => self.menu.reset(),
			KeyCode::Enter => {
				self.menu.select();
				crate::menu::process_menu_events(&mut self.menu, &mut self.command_queue);
			}
			KeyCode::Left | KeyCode::Char('h') => self.menu.left(),
			KeyCode::Right | KeyCode::Char('l') => self.menu.right(),
			KeyCode::Up | KeyCode::Char('k') => self.menu.up(),
			KeyCode::Down | KeyCode::Char('j') => self.menu.down(),
			_ => {}
		}
		self.needs_redraw = true;
	}

	fn handle_floating_escape(&mut self, key: &termina::event::KeyEvent) -> bool {
		if key.code != KeyCode::Escape {
			return false;
		}

		let FocusTarget::Buffer { window, .. } = self.focus else {
			return false;
		};

		if window == self.windows.base_id() {
			return false;
		}

		let Some(Window::Floating(floating)) = self.windows.get(window) else {
			return false;
		};

		if Some(window) == self.palette.window_id() {
			self.close_palette();
			self.needs_redraw = true;
			return true;
		}

		if floating.dismiss_on_blur {
			self.close_floating_window(window);
		}

		let base_buffer = self.base_window().focused_buffer;
		self.focus_view(base_buffer);
		self.needs_redraw = true;
		true
	}
}
