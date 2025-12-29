use evildoer_base::key::{Key, KeyCode, Modifiers};
use evildoer_manifest::{BindingMode, find_binding_resolved, resolve_action_id};

use crate::InputHandler;
use crate::types::{KeyResult, Mode};

impl InputHandler {
	pub(crate) fn handle_insert_key(&mut self, key: Key) -> KeyResult {
		if key.is_escape() {
			self.mode = Mode::Normal;
			self.reset_params();
			return KeyResult::ModeChange(Mode::Normal);
		}

		if key.is_backspace() {
			if let Some(id) = resolve_action_id("delete_back") {
				return KeyResult::ActionById {
					id,
					count: 1,
					extend: false,
					register: None,
				};
			}
			return KeyResult::Action {
				name: "delete_back",
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

		// Try insert-mode keybindings first
		if let Some(resolved) = find_binding_resolved(BindingMode::Insert, key) {
			let count = if self.count > 0 {
				self.count as usize
			} else {
				1
			};
			let extend = self.extend;
			let register = self.register;
			self.reset_params();
			return KeyResult::ActionById {
				id: resolved.action_id,
				count,
				extend,
				register,
			};
		}

		// Fall back to normal mode bindings for navigation keys
		let is_navigation_key =
			!matches!(key.code, KeyCode::Char(_)) || key.modifiers.ctrl || key.modifiers.alt;

		if is_navigation_key && let Some(resolved) = find_binding_resolved(BindingMode::Normal, key)
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
				id: resolved.action_id,
				count,
				extend,
				register,
			};
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
