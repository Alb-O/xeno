//! LSP menu input handling and navigation.
//!
//! Both completion and code action menus share the same navigation model: arrow keys
//! or j/k to move selection, Tab to accept, Escape to dismiss. This module handles
//! those key events and delegates to the appropriate handler ([`super::completion`]
//! or [`super::code_action`]) for the actual application.
//!
//! The menu state lives in [`LspMenuState`] overlay, which tracks which menu type
//! is active and its associated data (completion items or code actions).
//!
//! [`LspMenuState`]: super::types::LspMenuState

use xeno_primitives::{Key, KeyCode};

use super::types::{LspMenuKind, LspMenuState};
use crate::Editor;
use crate::completion::{CompletionState, SelectionIntent};

impl Editor {
	/// Handles key events when an LSP menu is active.
	///
	/// Returns `true` if the key was consumed by the menu, `false` otherwise.
	pub(crate) async fn handle_lsp_menu_key(&mut self, key: &Key) -> bool {
		let menu_kind = self.overlays().get::<LspMenuState>().and_then(|state: &LspMenuState| state.active()).cloned();
		let Some(menu_kind) = menu_kind else {
			return false;
		};

		let buffer_id = match &menu_kind {
			LspMenuKind::Completion { buffer_id, .. } => *buffer_id,
			LspMenuKind::CodeAction { buffer_id, .. } => *buffer_id,
		};
		if buffer_id != self.focused_view() {
			self.clear_lsp_menu();
			return false;
		}

		match key.code {
			KeyCode::Esc => {
				self.state.lsp.cancel_completion();
				if self.overlays().get::<CompletionState>().is_some_and(|s| s.active) {
					self.overlays_mut().get_or_default::<CompletionState>().suppressed = true;
				}
				self.clear_lsp_menu();
				return true;
			}
			KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
				self.move_lsp_menu_selection(-1);
				return true;
			}
			KeyCode::Down | KeyCode::Char('j') => {
				self.move_lsp_menu_selection(1);
				return true;
			}
			KeyCode::PageUp => {
				self.page_lsp_menu_selection(-1);
				return true;
			}
			KeyCode::PageDown => {
				self.page_lsp_menu_selection(1);
				return true;
			}
			KeyCode::Tab => {
				let state = self.overlays().get::<CompletionState>();
				let selected_idx = state.and_then(|completion| completion.selected_idx);
				if let Some(idx) = selected_idx {
					let raw_idx = lsp_completion_raw_index(state, idx);
					self.state.lsp.cancel_completion();
					self.clear_lsp_menu();
					match menu_kind {
						LspMenuKind::Completion { buffer_id, items } => {
							if let Some(item) = items.get(raw_idx).cloned() {
								self.apply_completion_item(buffer_id, item).await;
							}
						}
						LspMenuKind::CodeAction { buffer_id, actions } => {
							if let Some(action) = actions.get(idx).cloned() {
								self.apply_code_action_or_command(buffer_id, action).await;
							}
						}
					}
				} else {
					let state = self.overlays_mut().get_or_default::<CompletionState>();
					if !state.items.is_empty() {
						state.selected_idx = Some(0);
						state.selection_intent = SelectionIntent::Manual;
						state.ensure_selected_visible();
						self.state.frame.needs_redraw = true;
					}
				}
				return true;
			}
			KeyCode::Char('y') if key.modifiers.ctrl => {
				let state = self.overlays().get::<CompletionState>();
				let idx = state.and_then(|s| s.selected_idx).or_else(|| state.filter(|s| !s.items.is_empty()).map(|_| 0));
				let raw_idx = idx.map(|display_idx| lsp_completion_raw_index(state, display_idx));
				self.state.lsp.cancel_completion();
				self.clear_lsp_menu();
				if let Some(idx) = idx {
					match menu_kind {
						LspMenuKind::Completion { buffer_id, items } => {
							if let Some(item) = raw_idx.and_then(|mapped| items.get(mapped).cloned()) {
								self.apply_completion_item(buffer_id, item).await;
							}
						}
						LspMenuKind::CodeAction { buffer_id, actions } => {
							if let Some(action) = actions.get(idx).cloned() {
								self.apply_code_action_or_command(buffer_id, action).await;
							}
						}
					}
				}
				return true;
			}
			KeyCode::Enter => return false,
			_ => {}
		}

		false
	}

	fn move_lsp_menu_selection(&mut self, delta: isize) {
		let state = self.overlays_mut().get_or_default::<CompletionState>();
		if state.items.is_empty() {
			return;
		}
		let total = state.items.len();
		let current = state.selected_idx.unwrap_or(0) as isize;
		let mut next = current + delta;
		if next < 0 {
			next = total as isize - 1;
		} else if next as usize >= total {
			next = 0;
		}
		state.selected_idx = Some(next as usize);
		state.selection_intent = SelectionIntent::Manual;
		state.ensure_selected_visible();
		self.state.frame.needs_redraw = true;
	}

	fn page_lsp_menu_selection(&mut self, direction: isize) {
		let state = self.overlays_mut().get_or_default::<CompletionState>();
		if state.items.is_empty() {
			return;
		}
		let step = CompletionState::MAX_VISIBLE as isize;
		let delta = if direction >= 0 { step } else { -step };
		let total = state.items.len();
		let current = state.selected_idx.unwrap_or(0) as isize;
		let mut next = current + delta;
		if next < 0 {
			next = 0;
		} else if next as usize >= total {
			next = total.saturating_sub(1) as isize;
		}
		state.selected_idx = Some(next as usize);
		state.selection_intent = SelectionIntent::Manual;
		state.ensure_selected_visible();
		self.state.frame.needs_redraw = true;
	}
}

fn lsp_completion_raw_index(state: Option<&CompletionState>, display_idx: usize) -> usize {
	state
		.and_then(|completion| completion.lsp_display_to_raw.get(display_idx).copied())
		.unwrap_or(display_idx)
}

#[cfg(test)]
mod tests;
