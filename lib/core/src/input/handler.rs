use crate::ext::{BindingMode, find_binding};
use crate::input::types::{KeyResult, Mode};
use crate::key::{Key, KeyCode, MouseButton, MouseEvent, SpecialKey};

/// Manages input state and key processing.
#[derive(Debug, Clone)]
pub struct InputHandler {
	pub(crate) mode: Mode,
	pub(crate) count: u32,
	pub(crate) register: Option<char>,
	/// For commands that extend selection.
	pub(crate) extend: bool,
	/// Last search pattern for n/N repeat.
	///
	/// Tuple contains: (pattern, is_reverse)
	/// - `pattern`: The regex pattern being searched
	/// - `is_reverse`: True for backward search (?), false for forward (/)
	pub(crate) last_search: Option<(String, bool)>,
}

impl Default for InputHandler {
	fn default() -> Self {
		Self::new()
	}
}

impl InputHandler {
	pub fn new() -> Self {
		Self {
			mode: Mode::Normal,
			count: 0,
			register: None,
			extend: false,
			last_search: None,
		}
	}

	pub fn mode(&self) -> Mode {
		self.mode.clone()
	}

	pub fn mode_name(&self) -> &'static str {
		use crate::ext::PendingKind;
		match &self.mode {
			Mode::Normal => "NORMAL",
			Mode::Insert => "INSERT",
			Mode::Goto => "GOTO",
			Mode::View => "VIEW",
			Mode::Command { prompt, .. } => match prompt {
				':' => "COMMAND",
				'/' | '?' => "SEARCH",
				's' | 'S' => "SELECT",
				'k' | 'K' => "FILTER",
				'|' | '\\' | '!' | '@' => "SHELL",
				_ => "PROMPT",
			},
			Mode::PendingAction(kind) => match kind {
				PendingKind::FindChar { .. } | PendingKind::FindCharReverse { .. } => "FIND",
				PendingKind::ReplaceChar => "REPLACE",
				PendingKind::Object(_) => "OBJECT",
			},
		}
	}

	pub fn count(&self) -> u32 {
		self.count
	}

	pub fn effective_count(&self) -> u32 {
		if self.count == 0 { 1 } else { self.count }
	}

	pub fn register(&self) -> Option<char> {
		self.register
	}

	/// Get command line input if in command mode.
	pub fn command_line(&self) -> Option<(char, &str)> {
		match &self.mode {
			Mode::Command { prompt, input } => Some((*prompt, input.as_str())),
			_ => None,
		}
	}

	pub fn set_mode(&mut self, mode: Mode) {
		self.mode = mode.clone();
		if matches!(mode, Mode::Normal) {
			self.reset_params();
		}
	}

	/// Set the last search pattern (called by editor after executing search).
	pub fn set_last_search(&mut self, pattern: String, reverse: bool) {
		self.last_search = Some((pattern, reverse));
	}

	/// Get the last search pattern.
	pub fn last_search(&self) -> Option<(&str, bool)> {
		self.last_search.as_ref().map(|(p, r)| (p.as_str(), *r))
	}

	pub(crate) fn reset_params(&mut self) {
		self.count = 0;
		self.register = None;
		self.extend = false;
	}

	/// Process a key and return the result.
	pub fn handle_key(&mut self, key: Key) -> KeyResult {
		match &self.mode {
			Mode::Normal => self.handle_normal_key(key),
			Mode::Insert => self.handle_insert_key(key),
			Mode::Goto => self.handle_goto_key(key),
			Mode::View => self.handle_view_key(key),
			Mode::Command { prompt, input } => {
				let prompt = *prompt;
				let input = input.clone();
				self.handle_command_key(key, prompt, input)
			}
			Mode::PendingAction(kind) => {
				let kind = *kind;
				self.handle_pending_action_key(key, kind)
			}
		}
	}

	pub fn handle_mouse(&mut self, event: MouseEvent) -> KeyResult {
		match event {
			MouseEvent::Press {
				button: MouseButton::Left,
				row,
				col,
				..
			} => KeyResult::MouseClick {
				row,
				col,
				extend: self.extend,
			},
			MouseEvent::Drag {
				button: MouseButton::Left,
				row,
				col,
				..
			} => KeyResult::MouseDrag { row, col },
			MouseEvent::Scroll { direction, .. } => KeyResult::MouseScroll {
				direction,
				count: 1,
			},
			_ => KeyResult::Consumed,
		}
	}

	pub(crate) fn handle_goto_key(&mut self, key: Key) -> KeyResult {
		if matches!(key.code, KeyCode::Special(SpecialKey::Escape)) {
			self.mode = Mode::Normal;
			self.reset_params();
			return KeyResult::ModeChange(Mode::Normal);
		}

		let count = if self.count > 0 {
			self.count as usize
		} else {
			1
		};
		let extend = self.extend;
		let register = self.register;

		let key = self.extend_and_lower_if_shift(key);

		if let Some(binding) = find_binding(BindingMode::Goto, key) {
			self.mode = Mode::Normal;
			self.reset_params();
			return KeyResult::Action {
				name: binding.action,
				count,
				extend,
				register,
			};
		}

		self.mode = Mode::Normal;
		self.reset_params();
		KeyResult::Unhandled
	}

	pub(crate) fn handle_view_key(&mut self, key: Key) -> KeyResult {
		if matches!(key.code, KeyCode::Special(SpecialKey::Escape)) {
			self.mode = Mode::Normal;
			self.reset_params();
			return KeyResult::ModeChange(Mode::Normal);
		}

		let count = if self.count > 0 {
			self.count as usize
		} else {
			1
		};
		let extend = self.extend;
		let register = self.register;

		let key = key.normalize().drop_shift();

		// Try new keybinding registry first
		if let Some(binding) = find_binding(BindingMode::View, key) {
			self.mode = Mode::Normal;
			self.reset_params();
			return KeyResult::Action {
				name: binding.action,
				count,
				extend,
				register,
			};
		}

		self.mode = Mode::Normal;
		self.reset_params();
		KeyResult::Unhandled
	}

	/// If shift is held, set extend mode. For uppercase letters, use the uppercase binding
	/// if it exists, otherwise lowercase for binding lookup.
	pub(crate) fn extend_and_lower_if_shift(&mut self, key: Key) -> Key {
		// Handle Uppercase characters (Shift+Char or CapsLock+Char)
		// These always imply extend unless explicitly bound.
		if let KeyCode::Char(c) = key.code
			&& c.is_ascii_uppercase()
		{
			// Terminal may send uppercase without shift modifier (CapsLock or explicit uppercase key).
			// Only set extend if the Shift modifier was actually present; otherwise keep existing extend state.
			if key.modifiers.shift {
				self.extend = true;
			}

			// Check if uppercase key has its own binding (e.g. 'W')
			// We look up using the key without Shift modifier (since the char itself is uppercase)
			let lookup_key = key.drop_shift();

			if find_binding(BindingMode::Normal, lookup_key).is_some() {
				return lookup_key;
			}

			// If uppercase key not found, fallback to lowercase variant
			return Key {
				code: KeyCode::Char(c.to_ascii_lowercase()),
				modifiers: lookup_key.modifiers,
			};
		}

		// For non-uppercase chars, only handle explicit Shift modifier
		if !key.modifiers.shift {
			return key;
		}

		match key.code {
			// Punctuation/Symbols (e.g. ':', '!', '<')
			// These are produced by Shift, but we treat them as distinct keys without implicit extend.
			// Just drop the shift modifier so they match the bindings (which are usually Key::char(':')).
			KeyCode::Char(_) => {
				// Shift+char should still extend selection even if we drop the modifier for lookup.
				self.extend = true;
				key.drop_shift()
			}
			// Special keys (Arrows, PageUp, etc) with Shift -> Extend
			KeyCode::Special(_) => {
				self.extend = true;
				key.drop_shift()
			}
		}
	}
}
