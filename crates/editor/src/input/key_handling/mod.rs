//! Key event handling.

mod ops;
mod types;

use termina::event::KeyCode;
use types::ActionDispatch;
use xeno_input::input::KeyResult;
use xeno_primitives::{Key, Mode};

use crate::impls::Editor;

impl Editor {
	/// Processes a key event, routing to UI or input state machine.
	pub async fn handle_key(&mut self, key: termina::event::KeyEvent) -> bool {
		// UI global bindings (panels, focus, etc.)
		if self.state.ui.handle_global_key(&key) {
			if self.state.ui.take_wants_redraw() {
				self.state.frame.needs_redraw = true;
			}
			self.sync_focus_from_ui();
			self.interaction_on_buffer_edited();
			return false;
		}

		if self.state.ui.focused_panel_id().is_some() {
			let mut ui = std::mem::take(&mut self.state.ui);
			let _ = ui.handle_focused_key(self, key);
			if ui.take_wants_redraw() {
				self.state.frame.needs_redraw = true;
			}
			self.state.ui = ui;
			self.sync_focus_from_ui();
			self.interaction_on_buffer_edited();
			return false;
		}

		let quit = self.handle_key_active(key).await;
		self.interaction_on_buffer_edited();
		quit
	}

	/// Handles a key event when in active editing mode.
	pub(crate) async fn handle_key_active(&mut self, key: termina::event::KeyEvent) -> bool {
		use xeno_registry::HookEventData;
		use xeno_registry::hooks::{HookContext, emit as emit_hook};

		let old_mode = self.mode();
		#[cfg(feature = "lsp")]
		let old_buffer_id = self.focused_view();
		#[cfg(feature = "lsp")]
		let old_cursor = self.buffer().cursor;
		#[cfg(feature = "lsp")]
		let old_version = self.buffer().version();

		let mut interaction: crate::overlay::OverlayManager = std::mem::take(&mut self.state.overlay_system.interaction);
		let handled = interaction.handle_key(self, key);
		self.state.overlay_system.interaction = interaction;
		if handled {
			return false;
		}

		if self.state.overlay_system.interaction.is_open() && key.code == KeyCode::Enter {
			self.state.frame.pending_overlay_commit = true;
			self.state.frame.needs_redraw = true;
			return false;
		}

		#[cfg(feature = "lsp")]
		if self.handle_lsp_menu_key(&key).await {
			return false;
		}

		#[cfg(feature = "lsp")]
		if self.is_completion_trigger_key(&key) {
			self.trigger_lsp_completion(xeno_lsp::CompletionTrigger::Manual, None);
			return false;
		}
		let key_converted: Key = key.into();

		let result = self.buffer_mut().input.handle_key(key_converted);

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
					self.state.frame.needs_redraw = true;
				}
				KeyResult::ModeChange(new_mode) => {
					let leaving_insert = !matches!(new_mode, Mode::Insert);
					if new_mode != old_mode {
						let view = self.focused_view();
						self.notify_overlay_event(crate::overlay::LayerEvent::ModeChanged { view, mode: new_mode.clone() });
						emit_hook(&HookContext::new(HookEventData::ModeChange {
							old_mode,
							new_mode: new_mode.clone(),
						}))
						.await;
					}
					if leaving_insert {
						self.buffer_mut().clear_undo_group();
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
		self.update_lsp_completion_state(mode_change.as_ref(), old_buffer_id, old_cursor, old_version, inserted_char);

		quit
	}
}

#[cfg(test)]
mod tests {
	use termina::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, Modifiers};

	use crate::impls::Editor;

	fn key_enter() -> KeyEvent {
		KeyEvent {
			code: KeyCode::Enter,
			modifiers: Modifiers::NONE,
			kind: KeyEventKind::Press,
			state: KeyEventState::NONE,
		}
	}

	#[tokio::test]
	async fn enter_sets_pending_commit_and_pump_consumes() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(100, 40);
		assert!(editor.open_command_palette());

		let _ = editor.handle_key(key_enter()).await;
		assert!(editor.frame().pending_overlay_commit);

		let _ = editor.pump().await;
		assert!(!editor.state.overlay_system.interaction.is_open());
	}
}
