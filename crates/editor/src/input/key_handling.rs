//! Key event handling.
//!
//! Processing keyboard input and dispatching actions.

use termina::event::KeyCode;
use xeno_primitives::{Key, Mode, Selection};

use crate::impls::{Editor, FocusTarget};
use crate::input::KeyResult;
use crate::palette::PaletteState;
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
					self.invoke_action(action.name(), *count, *extend, *register, Some(*char_arg))
						.is_quit()
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

	/// Processes a key event, routing to UI or input state machine.
	pub async fn handle_key(&mut self, key: termina::event::KeyEvent) -> bool {
		// UI global bindings (panels, focus, etc.)
		if self.ui.handle_global_key(&key) {
			if self.ui.take_wants_redraw() {
				self.frame.needs_redraw = true;
			}
			self.sync_focus_from_ui();
			return false;
		}

		if self.ui.focused_panel_id().is_some() {
			let mut ui = std::mem::take(&mut self.ui);
			let _ = ui.handle_focused_key(self, key);
			if ui.take_wants_redraw() {
				self.frame.needs_redraw = true;
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
		#[cfg(feature = "lsp")]
		let old_buffer_id = self.focused_view();
		#[cfg(feature = "lsp")]
		let old_cursor = self.buffer().cursor;
		#[cfg(feature = "lsp")]
		let old_version = self.buffer().version();

		if self.palette_is_open() && key.code == KeyCode::Enter {
			self.execute_palette();
			self.frame.needs_redraw = true;
			return false;
		}
		#[cfg(feature = "lsp")]
		if self.prompt_is_open() && key.code == KeyCode::Enter {
			self.execute_prompt().await;
			self.frame.needs_redraw = true;
			return false;
		}

		#[cfg(feature = "lsp")]
		if self.handle_lsp_menu_key(&key).await {
			return false;
		}

		if self.handle_floating_escape(&key) {
			return false;
		}

		#[cfg(feature = "lsp")]
		if self.is_completion_trigger_key(&key) {
			self.trigger_lsp_completion(
				crate::lsp::completion_controller::CompletionTrigger::Manual,
				None,
			);
			return false;
		}
		let key: Key = key.into();

		let result = self.buffer_mut().input.handle_key(key);

		let mut quit = false;
		let mut handled = false;
		#[cfg(feature = "lsp")]
		let mut inserted_char = None;
		#[cfg(feature = "lsp")]
		let mut mode_change = None;

		if let ActionDispatch::Executed(action_quit) = self.dispatch_action(&result) {
			quit = action_quit;
			handled = true;
		}

		if !handled {
			match result {
				KeyResult::Pending { .. } => {
					self.frame.needs_redraw = true;
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
					#[cfg(feature = "lsp")]
					{
						mode_change = Some(new_mode);
					}
				}
				KeyResult::InsertChar(c) => {
					if !self.guard_readonly() {
						return false;
					}
					self.insert_text(&c.to_string());
					#[cfg(feature = "lsp")]
					{
						inserted_char = Some(c);
					}
				}
				KeyResult::Consumed | KeyResult::Unhandled => {}
				KeyResult::Quit => {
					quit = true;
				}
				KeyResult::MouseClick { row, col, extend } => {
					let view_area = self.focused_view_area();
					let local_row = row.saturating_sub(view_area.y);
					let local_col = col.saturating_sub(view_area.x);
					self.handle_mouse_click_local(local_row, local_col, extend);
				}
				KeyResult::MouseDrag { row, col } => {
					let view_area = self.focused_view_area();
					let local_row = row.saturating_sub(view_area.y);
					let local_col = col.saturating_sub(view_area.x);
					self.handle_mouse_drag_local(local_row, local_col);
				}
				KeyResult::MouseScroll { direction, count } => {
					self.handle_mouse_scroll(direction, count);
				}
				_ => unreachable!(),
			}
		}

		#[cfg(feature = "lsp")]
		self.update_lsp_completion_state(
			mode_change.as_ref(),
			old_buffer_id,
			old_cursor,
			old_version,
			inserted_char,
		);

		quit
	}

	/// Updates LSP completion and signature help state after a key event.
	///
	/// Manages the completion menu lifecycle based on mode changes, focus changes,
	/// content modifications, and cursor movement.
	///
	/// - **Mode change away from insert**: Cancels all LSP features and closes menus.
	/// - **Cursor before completion start**: Closes the menu (user backspaced past word).
	/// - **Focus change**: Clears all LSP state for the old buffer.
	/// - **Content change**: Triggers completion (`Typing`, 80ms debounce).
	/// - **Cursor-only change**: Does not trigger completion requests.
	#[cfg(feature = "lsp")]
	fn update_lsp_completion_state(
		&mut self,
		mode_change: Option<&xeno_primitives::Mode>,
		old_buffer_id: crate::buffer::BufferId,
		old_cursor: usize,
		old_version: u64,
		inserted_char: Option<char>,
	) {
		use crate::CompletionState;
		use crate::lsp::completion_controller::CompletionTrigger;

		if let Some(new_mode) = mode_change
			&& !matches!(new_mode, xeno_primitives::Mode::Insert)
		{
			self.completion_controller.cancel();
			self.cancel_signature_help();
			self.clear_lsp_menu();
		}

		let focus_changed = old_buffer_id != self.focused_view();
		let cursor_changed = old_cursor != self.buffer().cursor;
		let content_changed = old_version != self.buffer().version();

		let cursor = self.buffer().cursor;
		let menu_active = self
			.overlays
			.get::<CompletionState>()
			.is_some_and(|s| s.active);
		let replace_start = self
			.overlays
			.get::<CompletionState>()
			.map(|s| s.replace_start)
			.unwrap_or(0);

		if cursor < replace_start {
			self.completion_controller.cancel();
			self.clear_lsp_menu();
		} else if menu_active && cursor_changed {
			self.frame.needs_redraw = true;
		}

		if focus_changed {
			self.completion_controller.cancel();
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
		}
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

		let palette_window = self
			.overlays
			.get::<PaletteState>()
			.and_then(|p| p.window_id());
		if Some(window) == palette_window {
			self.close_palette();
			self.frame.needs_redraw = true;
			return true;
		}

		#[cfg(feature = "lsp")]
		{
			let prompt_window = self
				.overlays
				.get::<crate::prompt::PromptState>()
				.and_then(|state| state.window_id());
			if Some(window) == prompt_window {
				self.close_prompt();
				self.frame.needs_redraw = true;
				return true;
			}
		}

		if floating.dismiss_on_blur {
			self.close_floating_window(window);
		}

		let base_buffer = self.base_window().focused_buffer;
		self.focus_view(base_buffer);
		self.frame.needs_redraw = true;
		true
	}
}
