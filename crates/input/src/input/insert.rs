//! Insert mode key handling.

use xeno_keymap_core::ToKeyMap;
use xeno_primitives::key::{Key, KeyCode, Modifiers};
use xeno_registry::actions::BindingMode;
use xeno_registry::{KeymapSnapshot, LookupOutcome};

use super::InputHandler;
use super::types::{KeyResult, Mode};

impl InputHandler {
	/// Processes a key press in insert mode.
	///
	/// Text characters (unmodified or Shift-only, plus Space/Enter/Tab) insert
	/// immediately without keymap lookup. All other keys (Ctrl/Alt combos,
	/// function keys, Backspace, Delete, Escape) enter the multi-key sequence
	/// mechanism for Insert-mode bindings.
	pub(crate) fn handle_insert_key(&mut self, key: Key, registry: &KeymapSnapshot) -> KeyResult {
		// Escape always exits to Normal.
		if key.is_escape() {
			self.mode = Mode::Normal;
			self.reset_params();
			return KeyResult::ModeChange(Mode::Normal);
		}

		// Determine if this is a "text input" key that should insert immediately.
		let is_text_input = match key.code {
			KeyCode::Char(_) if key.modifiers.is_empty() || key.modifiers == Modifiers::SHIFT => true,
			KeyCode::Space | KeyCode::Enter | KeyCode::Tab => self.key_sequence.is_empty(),
			_ => false,
		};

		if is_text_input && self.key_sequence.is_empty() {
			return match key.code {
				KeyCode::Char(c) => KeyResult::InsertChar(c),
				KeyCode::Space => KeyResult::InsertChar(' '),
				KeyCode::Enter => KeyResult::InsertChar('\n'),
				KeyCode::Tab => KeyResult::InsertChar('\t'),
				_ => unreachable!(),
			};
		}

		// Non-text key (or continuation of a pending sequence): push into the
		// key_sequence accumulator and look up Insert-mode bindings.
		let key = self.canonicalize_insert_key(key);

		let node = match key.to_keymap() {
			Ok(n) => n,
			Err(_) => return KeyResult::Consumed,
		};

		self.key_sequence.push(node);
		let result = registry.lookup(BindingMode::Insert, &self.key_sequence);

		match result {
			LookupOutcome::Match(entry) => self.consume_binding(entry),
			LookupOutcome::Pending { .. } => KeyResult::Pending {
				keys_so_far: self.key_sequence.len(),
			},
			LookupOutcome::None => {
				// Unknown prefix — clear pending and consume (don't insert garbage).
				self.key_sequence.clear();
				KeyResult::Consumed
			}
		}
	}

	/// Canonicalizes a key for Insert-mode keymap lookup.
	///
	/// Shift+lowercase letter normalizes to the uppercase character.
	/// Shift on non-char keys is dropped (extend semantics preserved).
	fn canonicalize_insert_key(&mut self, key: Key) -> Key {
		if let KeyCode::Char(c) = key.code {
			if key.modifiers.shift {
				if c.is_ascii_lowercase() { key.normalize() } else { key.drop_shift() }
			} else {
				key
			}
		} else if key.modifiers.shift {
			self.extend = true;
			key.drop_shift()
		} else {
			key
		}
	}
}
