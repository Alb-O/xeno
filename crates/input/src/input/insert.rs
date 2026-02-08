//! Insert mode key handling.

use xeno_keymap_core::ToKeyMap;
use xeno_primitives::key::{Key, KeyCode, Modifiers};
use xeno_registry::actions::{BindingMode, keys as actions};
use xeno_registry::{LookupResult, get_keymap_registry, resolve_action_key};

use super::InputHandler;
use super::types::{KeyResult, Mode};

impl InputHandler {
	/// Processes a key press in insert mode.
	///
	/// Resolution order: escape → direct actions (backspace/delete) → shift
	/// canonicalization → insert-mode bindings → normal-mode fallback for
	/// navigation keys → literal character insertion.
	pub(crate) fn handle_insert_key(&mut self, key: Key) -> KeyResult {
		if key.is_escape() {
			self.mode = Mode::Normal;
			self.reset_params();
			return KeyResult::ModeChange(Mode::Normal);
		}

		let direct_action = if key.is_backspace() {
			Some(actions::delete_back)
		} else if key.is_delete() {
			Some(actions::delete_forward)
		} else {
			None
		};
		if let Some(action_key) = direct_action {
			let id = resolve_action_key(action_key).expect("action not registered");
			return KeyResult::ActionById {
				id,
				count: 1,
				extend: false,
				register: None,
			};
		}

		let key = if let KeyCode::Char(c) = key.code {
			if key.modifiers.shift {
				if c.is_ascii_lowercase() {
					key.normalize()
				} else {
					key.drop_shift()
				}
			} else {
				key
			}
		} else if key.modifiers.shift {
			self.extend = true;
			key.drop_shift()
		} else {
			key
		};

		let registry = get_keymap_registry();

		if let Ok(node) = key.to_keymap() {
			if let LookupResult::Match(entry) =
				registry.lookup(BindingMode::Insert, std::slice::from_ref(&node))
			{
				return self.consume_action(entry.action_id);
			}

			let is_navigation_key =
				!matches!(key.code, KeyCode::Char(_)) || key.modifiers.ctrl || key.modifiers.alt;

			if is_navigation_key
				&& let LookupResult::Match(entry) = registry.lookup(BindingMode::Normal, &[node])
			{
				return self.consume_action(entry.action_id);
			}
		}

		match key.code {
			KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == Modifiers::SHIFT => {
				KeyResult::InsertChar(c)
			}
			KeyCode::Space => KeyResult::InsertChar(' '),
			KeyCode::Enter => KeyResult::InsertChar('\n'),
			KeyCode::Tab => KeyResult::InsertChar('\t'),
			_ => KeyResult::Consumed,
		}
	}
}
