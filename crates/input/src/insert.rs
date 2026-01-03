//! Insert mode key handling.

use xeno_base::key::{Key, KeyCode, Modifiers};
use xeno_core::keymap_registry::LookupResult;
use xeno_core::{get_keymap_registry, resolve_action_key};
use xeno_keymap::ToKeyMap;
use xeno_registry::BindingMode;
use xeno_registry::actions::keys as actions;

use crate::InputHandler;
use crate::types::{KeyResult, Mode};

impl InputHandler {
	/// Processes a key press in insert mode.
	pub(crate) fn handle_insert_key(&mut self, key: Key) -> KeyResult {
		if key.is_escape() {
			self.mode = Mode::Normal;
			self.reset_params();
			return KeyResult::ModeChange(Mode::Normal);
		}

		if key.is_backspace() {
			let id = resolve_action_key(actions::delete_back)
				.expect("delete_back action not registered");
			return KeyResult::ActionById {
				id,
				count: 1,
				extend: false,
				register: None,
			};
		}

		// Normalize Shift+Letter -> Uppercase Letter (no shift)
		let key = if key.modifiers.shift
			&& let KeyCode::Char(c) = key.code
			&& c.is_ascii_lowercase()
		{
			key.normalize()
		} else {
			key
		};

		let registry = get_keymap_registry();

		// Try insert-mode keybindings first
		if let Ok(node) = key.to_keymap() {
			if let LookupResult::Match(entry) =
				registry.lookup(BindingMode::Insert, std::slice::from_ref(&node))
			{
				let count = if self.count > 0 {
					self.count as usize
				} else {
					1
				};
				let extend = self.extend;
				let register = self.register;
				self.reset_params();
				return KeyResult::ActionById {
					id: entry.action_id,
					count,
					extend,
					register,
				};
			}

			// Fall back to normal mode bindings for navigation keys
			let is_navigation_key =
				!matches!(key.code, KeyCode::Char(_)) || key.modifiers.ctrl || key.modifiers.alt;

			if is_navigation_key
				&& let LookupResult::Match(entry) = registry.lookup(BindingMode::Normal, &[node])
			{
				let count = if self.count > 0 {
					self.count as usize
				} else {
					1
				};
				let extend = self.extend;
				let register = self.register;
				self.reset_params();
				return KeyResult::ActionById {
					id: entry.action_id,
					count,
					extend,
					register,
				};
			}
		}

		// Regular character insertion
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
