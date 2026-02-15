//! Key event handling.

mod ops;
mod types;

use types::ActionDispatch;
use xeno_input::input::KeyResult;
use xeno_primitives::{Key, KeyCode, Mode};

use crate::Editor;

impl Editor {
	/// Processes a key event, routing to UI or input state machine.
	pub async fn handle_key(&mut self, key: Key) -> bool {
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
	pub(crate) async fn handle_key_active(&mut self, key: Key) -> bool {
		use xeno_registry::HookEventData;
		use xeno_registry::hooks::{HookContext, emit as emit_hook};

		let old_mode = self.mode();
		#[cfg(feature = "lsp")]
		let old_buffer_id = self.focused_view();
		#[cfg(feature = "lsp")]
		let old_cursor = self.buffer().cursor;
		#[cfg(feature = "lsp")]
		let old_version = self.buffer().version();

		let mut interaction = self.state.overlay_system.take_interaction();
		let handled = interaction.handle_key(self, key);
		self.state.overlay_system.restore_interaction(interaction);
		if handled {
			return false;
		}
		let mut layers = std::mem::take(self.state.overlay_system.layers_mut());
		let handled = layers.handle_key(self, key);
		*self.state.overlay_system.layers_mut() = layers;
		if handled {
			return false;
		}

		if self.state.overlay_system.interaction().is_open() && key.code == KeyCode::Enter {
			self.state.frame.pending_overlay_commit = true;
			self.state.frame.needs_redraw = true;
			return false;
		}

		#[cfg(feature = "lsp")]
		if self.handle_lsp_menu_key(&key).await {
			return false;
		}

		if self.handle_snippet_session_key(&key) {
			return false;
		}

		#[cfg(feature = "lsp")]
		if self.is_completion_trigger_key(&key) {
			self.trigger_lsp_completion(xeno_lsp::CompletionTrigger::Manual, None);
			return false;
		}
		let keymap = self.effective_keymap();

		let result = self.buffer_mut().input.handle_key_with_registry(key, &keymap);

		let mut quit = false;
		let mut handled = false;
		#[cfg(feature = "lsp")]
		let mut inserted_char = None;
		#[cfg(feature = "lsp")]
		let mut mode_change = None;

		if let ActionDispatch::Executed(action_result) = self.dispatch_action(&result) {
			quit = action_result.is_quit();
			handled = true;

			if !action_result.is_quit()
				&& let Some(action_name) = action_name_from_key_result(&result)
				&& let Some(hook_result) = self.emit_action_post_hook(action_name, &action_result).await
			{
				quit = hook_result.is_quit();
			}
		}

		if !handled && let KeyResult::Invocation { ref inv } = result {
			let inv_result = self.run_invocation(inv.clone(), crate::types::InvocationPolicy::enforcing()).await;
			quit = inv_result.is_quit();
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
							old_mode: old_mode.clone(),
							new_mode: new_mode.clone(),
						}))
						.await;

						if let Some(hook_result) = self.emit_mode_change_hook(&old_mode, &new_mode).await
							&& hook_result.is_quit()
						{
							quit = true;
						}
					}
					if leaving_insert {
						self.cancel_snippet_session();
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
					let text = c.to_string();
					if !self.snippet_replace_mode_insert(&text) {
						self.insert_text(&text);
					}
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

fn action_name_from_key_result(result: &KeyResult) -> Option<String> {
	match result {
		KeyResult::ActionById { id, .. } | KeyResult::ActionByIdWithChar { id, .. } => {
			xeno_registry::ACTIONS.get_by_id(*id).map(|action| action.name_str().to_string())
		}
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;
	use std::sync::Arc;

	use xeno_keymap_core::parser::parse_seq;
	use xeno_primitives::{Key, KeyCode};
	use xeno_registry::actions::{ActionEntry, BindingMode};
	use xeno_registry::config::UnresolvedKeys;
	use xeno_registry::core::index::Snapshot;
	use xeno_registry::{ActionId, DenseId, LookupResult, RegistryEntry};

	use crate::Editor;

	fn key_enter() -> Key {
		Key::new(KeyCode::Enter)
	}

	fn mode_name(mode: BindingMode) -> &'static str {
		match mode {
			BindingMode::Normal => "normal",
			BindingMode::Insert => "insert",
			BindingMode::Match => "match",
			BindingMode::Space => "space",
		}
	}

	fn sample_binding(actions: &Snapshot<ActionEntry, ActionId>) -> Option<(BindingMode, String, ActionId, ActionId, String)> {
		for (idx, action_entry) in actions.table.iter().enumerate() {
			let source_id = ActionId::from_u32(idx as u32);
			for binding in action_entry.bindings.iter() {
				if parse_seq(&binding.keys).is_err() {
					continue;
				}

				let Some((target_idx, target_entry)) = actions.table.iter().enumerate().find(|(target_idx, _)| *target_idx != idx) else {
					continue;
				};

				let target_id = ActionId::from_u32(target_idx as u32);
				let target_id_str = actions.interner.resolve(target_entry.id()).to_string();
				return Some((binding.mode, binding.keys.to_string(), source_id, target_id, target_id_str));
			}
		}
		None
	}

	fn lookup_action_id(index: &xeno_registry::KeymapIndex, mode: BindingMode, key_seq: &str) -> ActionId {
		let keys = parse_seq(key_seq).expect("key sequence should parse");
		match index.lookup(mode, &keys) {
			LookupResult::Match(entry) => entry.action_id().expect("expected action binding"),
			_ => panic!("expected a complete keybinding match"),
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
		assert!(!editor.state.overlay_system.interaction().is_open());
	}

	#[test]
	fn effective_keymap_applies_overrides_and_invalidates_cache() {
		let mut editor = Editor::new_scratch();
		let actions = xeno_registry::db::ACTIONS.snapshot();
		let (mode, key_seq, base_id, target_id, target_id_str) = sample_binding(&actions).expect("registry should contain at least one binding");

		let keymap_before = editor.effective_keymap();
		assert_eq!(lookup_action_id(&keymap_before, mode, &key_seq), base_id);

		let mut mode_overrides = HashMap::new();
		mode_overrides.insert(key_seq.clone(), xeno_registry::Invocation::action(&target_id_str));
		let mut modes = HashMap::new();
		modes.insert(mode_name(mode).to_string(), mode_overrides);
		editor.set_key_overrides(Some(UnresolvedKeys { modes }));

		let keymap_after = editor.effective_keymap();
		assert!(!Arc::ptr_eq(&keymap_before, &keymap_after));
		assert_eq!(lookup_action_id(&keymap_after, mode, &key_seq), target_id);
	}

	#[test]
	fn effective_keymap_continuations_include_override() {
		let mut editor = Editor::new_scratch();
		let actions = xeno_registry::db::ACTIONS.snapshot();
		let (_mode, _key_seq, _base_id, _target_id, target_id_str) = sample_binding(&actions).expect("registry should contain at least one binding");

		let base = editor.effective_keymap();
		let mut chosen_prefix = None;
		for action in &*actions.table {
			for binding in action.bindings.iter().filter(|b| b.mode == BindingMode::Normal) {
				let Ok(nodes) = parse_seq(&binding.keys) else {
					continue;
				};
				if nodes.len() < 2 {
					continue;
				}

				let prefix = nodes[0].to_string();
				let prefix_nodes = parse_seq(&prefix).expect("prefix should parse");
				let existing: std::collections::HashSet<String> = base
					.continuations_with_kind(BindingMode::Normal, &prefix_nodes)
					.into_iter()
					.map(|c| c.key.to_string())
					.collect();

				let candidate = ('a'..='z').map(|c| c.to_string()).find(|k| !existing.contains(k));
				if let Some(candidate) = candidate {
					chosen_prefix = Some((prefix, candidate));
					break;
				}
			}
			if chosen_prefix.is_some() {
				break;
			}
		}

		let (prefix, candidate) = chosen_prefix.expect("expected a prefix with an available continuation slot");
		let full_key = format!("{prefix} {candidate}");

		let mut normal = HashMap::new();
		normal.insert(full_key, xeno_registry::Invocation::action(&target_id_str));
		let mut modes = HashMap::new();
		modes.insert("normal".to_string(), normal);
		editor.set_key_overrides(Some(UnresolvedKeys { modes }));

		let keymap = editor.effective_keymap();
		let prefix_nodes = parse_seq(&prefix).expect("prefix should parse");
		let continuations: std::collections::HashSet<String> = keymap
			.continuations_with_kind(BindingMode::Normal, &prefix_nodes)
			.into_iter()
			.map(|c| c.key.to_string())
			.collect();

		assert!(continuations.contains(&candidate));
	}

	#[test]
	fn invalid_override_keeps_base_binding() {
		let mut editor = Editor::new_scratch();
		let actions = xeno_registry::db::ACTIONS.snapshot();
		let (mode, key_seq, base_id, _target_id, _target_id_str) = sample_binding(&actions).expect("registry should contain at least one binding");

		let mut mode_overrides = HashMap::new();
		mode_overrides.insert(key_seq.clone(), xeno_registry::Invocation::action("does-not-exist"));
		let mut modes = HashMap::new();
		modes.insert(mode_name(mode).to_string(), mode_overrides);
		editor.set_key_overrides(Some(UnresolvedKeys { modes }));

		let keymap = editor.effective_keymap();
		assert_eq!(lookup_action_id(&keymap, mode, &key_seq), base_id);
	}
}
