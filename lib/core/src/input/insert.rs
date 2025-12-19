use crate::ext::{BindingMode, find_binding};
use crate::input::InputHandler;
use crate::input::types::{KeyResult, Mode};
use crate::key::{Key, KeyCode, Modifiers, SpecialKey};

impl InputHandler {
	pub(crate) fn handle_insert_key(&mut self, key: Key) -> KeyResult {
		if matches!(key.code, KeyCode::Special(SpecialKey::Escape)) {
			self.mode = Mode::Normal;
			self.reset_params();
			return KeyResult::ModeChange(Mode::Normal);
		}

		// Backspace in insert mode deletes backward
		if matches!(key.code, KeyCode::Special(SpecialKey::Backspace)) {
			return KeyResult::Action {
				name: "delete_back",
				count: 1,
				extend: false,
				register: None,
			};
		}

		// Normalize Shift+Letter -> Uppercase Letter (no shift)
		// This ensures typing Shift+a produces 'A' even if terminal sends 'a'+Shift.
		let key = if key.modifiers.shift
			&& let KeyCode::Char(c) = key.code
			&& c.is_ascii_lowercase()
		{
			key.normalize()
		} else {
			key
		};

		// Try insert-mode keybindings first
		if let Some(binding) = find_binding(BindingMode::Insert, key) {
			let count = if self.count > 0 {
				self.count as usize
			} else {
				1
			};
			let extend = self.extend;
			let register = self.register;
			self.reset_params();
			return KeyResult::Action {
				name: binding.action,
				count,
				extend,
				register,
			};
		}

		// Fall back to normal mode bindings only for non-character keys (navigation)
		let is_navigation_key =
			matches!(key.code, KeyCode::Special(_)) || key.modifiers.ctrl || key.modifiers.alt;

		if is_navigation_key && let Some(binding) = find_binding(BindingMode::Normal, key) {
			let count = if self.count > 0 {
				self.count as usize
			} else {
				1
			};
			let extend = self.extend;
			let register = self.register;
			self.reset_params();
			return KeyResult::Action {
				name: binding.action,
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
			KeyCode::Special(SpecialKey::Enter) => KeyResult::InsertChar('\n'),
			KeyCode::Special(SpecialKey::Tab) => KeyResult::InsertChar('\t'),
			_ => KeyResult::Consumed,
		}
	}
}
