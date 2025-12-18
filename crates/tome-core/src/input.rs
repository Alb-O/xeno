//! Input handling with mode stack and count prefixes.
//! New action-only path (legacy Command path removed).

use crate::ext::{BindingMode, ObjectSelectionKind, PendingKind, find_binding};
use crate::key::{Key, KeyCode, Modifiers, MouseButton, MouseEvent, ScrollDirection, SpecialKey};

/// Editor mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Mode {
	#[default]
	Normal,
	Insert,
	Goto,
	View,
	/// Command line input mode (for `:`, `/`, `?`, regex, pipe prompts).
	Command {
		prompt: char,
		input: String,
	},
	/// Waiting for character input to complete an action.
	PendingAction(PendingKind),
}

/// Result of processing a key.
#[derive(Debug, Clone)]
pub enum KeyResult {
	/// An action to execute (string-based system).
	Action {
		name: &'static str,
		count: usize,
		extend: bool,
		register: Option<char>,
	},
	/// An action with a character argument (from pending completion).
	ActionWithChar {
		name: &'static str,
		count: usize,
		extend: bool,
		register: Option<char>,
		char_arg: char,
	},
	/// Mode changed (to show in status).
	ModeChange(Mode),
	/// Key was consumed but no action needed.
	Consumed,
	/// Key was not handled.
	Unhandled,
	/// Insert a character (in insert mode).
	InsertChar(char),
	/// Execute a command-line command (from `:` prompt).
	ExecuteCommand(String),
	/// Execute a search (from `/` or `?` prompt).
	ExecuteSearch { pattern: String, reverse: bool },
	/// Select regex matches within selection (from `s` prompt).
	SelectRegex { pattern: String },
	/// Split selection on regex (from `S` prompt).
	SplitRegex { pattern: String },
	/// Keep selections matching regex (from `alt-k` prompt).
	KeepMatching { pattern: String },
	/// Keep selections not matching regex (from `alt-K` prompt).
	KeepNotMatching { pattern: String },
	/// Pipe selection through shell command, replace with output.
	PipeReplace { command: String },
	/// Pipe selection through shell command, ignore output.
	PipeIgnore { command: String },
	/// Insert shell command output before selection.
	InsertOutput { command: String },
	/// Append shell command output after selection.
	AppendOutput { command: String },
	/// Request to quit.
	Quit,
	/// Mouse click at screen coordinates.
	MouseClick { row: u16, col: u16, extend: bool },
	/// Mouse drag to screen coordinates (extend selection).
	MouseDrag { row: u16, col: u16 },
	/// Mouse scroll.
	MouseScroll {
		direction: ScrollDirection,
		count: usize,
	},
}

/// Manages input state and key processing.
#[derive(Debug, Clone)]
pub struct InputHandler {
	mode: Mode,
	count: u32,
	register: Option<char>,
	/// For commands that extend selection.
	extend: bool,
	/// Last search pattern for n/N repeat.
	///
	/// Tuple contains: (pattern, is_reverse)
	/// - `pattern`: The regex pattern being searched
	/// - `is_reverse`: True for backward search (?), false for forward (/)
	last_search: Option<(String, bool)>,
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

	fn reset_params(&mut self) {
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
			MouseEvent::Press {
				button: MouseButton::Right,
				..
			}
			| MouseEvent::Press {
				button: MouseButton::Middle,
				..
			}
			| MouseEvent::Drag {
				button: MouseButton::Right,
				..
			}
			| MouseEvent::Drag {
				button: MouseButton::Middle,
				..
			}
			| MouseEvent::Release { .. } => KeyResult::Consumed,
		}
	}

	fn handle_normal_key(&mut self, key: Key) -> KeyResult {
		if let Some(digit) = key.as_digit()
			&& (digit != 0 || self.count > 0)
		{
			self.count = self.count.saturating_mul(10).saturating_add(digit);
			return KeyResult::Consumed;
		}

		let key = self.extend_and_lower_if_shift(key);

		if let Some(binding) = find_binding(BindingMode::Normal, key) {
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

		self.reset_params();
		KeyResult::Unhandled
	}

	fn handle_insert_key(&mut self, key: Key) -> KeyResult {
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

	fn handle_goto_key(&mut self, key: Key) -> KeyResult {
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

	fn handle_view_key(&mut self, key: Key) -> KeyResult {
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

	fn handle_command_key(&mut self, key: Key, prompt: char, mut input: String) -> KeyResult {
		let key = if key.modifiers.shift
			&& let KeyCode::Char(c) = key.code
			&& c.is_ascii_lowercase()
		{
			key.normalize()
		} else {
			key
		};

		match key.code {
			KeyCode::Special(SpecialKey::Escape) => {
				self.mode = Mode::Normal;
				self.reset_params();
				KeyResult::ModeChange(Mode::Normal)
			}

			KeyCode::Special(SpecialKey::Enter) => {
				self.mode = Mode::Normal;
				self.reset_params();

				if input.is_empty() {
					return KeyResult::Consumed;
				}

				match prompt {
					':' => KeyResult::ExecuteCommand(input),
					'/' => KeyResult::ExecuteSearch {
						pattern: input,
						reverse: false,
					},
					'?' => KeyResult::ExecuteSearch {
						pattern: input,
						reverse: true,
					},
					's' => KeyResult::SelectRegex { pattern: input },
					'S' => KeyResult::SplitRegex { pattern: input },
					'k' => KeyResult::KeepMatching { pattern: input },
					'K' => KeyResult::KeepNotMatching { pattern: input },
					'|' => KeyResult::PipeReplace { command: input },
					'\\' => KeyResult::PipeIgnore { command: input },
					'!' => KeyResult::InsertOutput { command: input },
					'@' => KeyResult::AppendOutput { command: input },
					_ => KeyResult::Consumed,
				}
			}

			KeyCode::Special(SpecialKey::Backspace) => {
				if input.is_empty() {
					self.mode = Mode::Normal;
					self.reset_params();
					KeyResult::ModeChange(Mode::Normal)
				} else {
					input.pop();
					self.mode = Mode::Command { prompt, input };
					KeyResult::Consumed
				}
			}

			KeyCode::Char(c) if key.modifiers.is_empty() => {
				input.push(c);
				self.mode = Mode::Command { prompt, input };
				KeyResult::Consumed
			}

			KeyCode::Char(' ') => {
				input.push(' ');
				self.mode = Mode::Command { prompt, input };
				KeyResult::Consumed
			}

			_ => KeyResult::Consumed,
		}
	}

	fn handle_pending_action_key(&mut self, key: Key, pending: PendingKind) -> KeyResult {
		let key = if key.modifiers.shift
			&& let KeyCode::Char(c) = key.code
			&& c.is_ascii_lowercase()
		{
			key.normalize()
		} else {
			key
		};

		match pending {
			PendingKind::FindChar { inclusive: _ } => match key.code {
				KeyCode::Char(ch) => {
					let count = self.effective_count() as usize;
					let extend = self.extend;
					let register = self.register;
					self.reset_params();
					KeyResult::ActionWithChar {
						name: "find_char",
						count,
						extend,
						register,
						char_arg: ch,
					}
				}
				KeyCode::Special(SpecialKey::Escape) => {
					self.mode = Mode::Normal;
					self.reset_params();
					KeyResult::ModeChange(Mode::Normal)
				}
				_ => KeyResult::Consumed,
			},

			PendingKind::FindCharReverse { inclusive: _ } => match key.code {
				KeyCode::Char(ch) => {
					let count = self.effective_count() as usize;
					let extend = self.extend;
					let register = self.register;
					self.reset_params();
					KeyResult::ActionWithChar {
						name: "find_char_reverse",
						count,
						extend,
						register,
						char_arg: ch,
					}
				}
				KeyCode::Special(SpecialKey::Escape) => {
					self.mode = Mode::Normal;
					self.reset_params();
					KeyResult::ModeChange(Mode::Normal)
				}
				_ => KeyResult::Consumed,
			},

			PendingKind::ReplaceChar => match key.code {
				KeyCode::Char(ch) => {
					let count = self.effective_count() as usize;
					let extend = self.extend;
					let register = self.register;
					self.reset_params();
					KeyResult::ActionWithChar {
						name: "replace_char",
						count,
						extend,
						register,
						char_arg: ch,
					}
				}
				KeyCode::Special(SpecialKey::Escape) => {
					self.mode = Mode::Normal;
					self.reset_params();
					KeyResult::ModeChange(Mode::Normal)
				}
				_ => KeyResult::Consumed,
			},

			PendingKind::Object(selection) => match key.code {
				KeyCode::Char(ch) => {
					let count = self.effective_count() as usize;
					let extend = self.extend;
					let register = self.register;
					self.reset_params();

					let kind = match selection {
						ObjectSelectionKind::Inner => Some("select_object_inner"),
						ObjectSelectionKind::Around => Some("select_object_around"),
						ObjectSelectionKind::ToStart => Some("select_object_to_start"),
						ObjectSelectionKind::ToEnd => Some("select_object_to_end"),
					};

					match kind {
						Some(action) => KeyResult::ActionWithChar {
							name: action,
							count,
							extend,
							register,
							char_arg: ch,
						},
						None => KeyResult::Consumed,
					}
				}
				KeyCode::Special(SpecialKey::Escape) => {
					self.mode = Mode::Normal;
					self.reset_params();
					KeyResult::ModeChange(Mode::Normal)
				}
				_ => KeyResult::Consumed,
			},
		}
	}

	/// If shift is held, set extend mode. For uppercase letters, use the uppercase binding
	/// if it exists, otherwise lowercase for binding lookup.
	fn extend_and_lower_if_shift(&mut self, key: Key) -> Key {
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

			// Fallback to lowercase
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

// Tests for the action-only input handler are intentionally minimal for now.
#[cfg(test)]
mod tests {
	use super::*;
	use crate::{KeyCode, Modifiers};

	#[test]
	fn test_digit_count_accumulates() {
		let mut h = InputHandler::new();
		h.handle_key(Key::char('2'));
		h.handle_key(Key::char('3'));
		assert_eq!(h.effective_count(), 23);
	}

	fn key_with_shift(c: char) -> Key {
		Key {
			code: KeyCode::Char(c),
			modifiers: Modifiers {
				shift: true,
				..Modifiers::NONE
			},
		}
	}

	#[test]
	fn test_word_motion_sets_extend_with_shift() {
		let mut h = InputHandler::new();
		let res = h.handle_key(key_with_shift('w'));
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "next_word_start");
				assert!(extend);
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_word_motion_no_shift_not_extend() {
		let mut h = InputHandler::new();
		let res = h.handle_key(Key::char('w'));
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "next_word_start");
				assert!(!extend);
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	/// Simulates what the terminal sends: Shift+w comes as uppercase 'W' with shift modifier.
	/// (We no longer call normalize() in the From<KeyEvent> impl)
	fn key_shifted_uppercase(c: char) -> Key {
		// Terminal sends: Char(uppercase) + Shift modifier
		Key {
			code: KeyCode::Char(c.to_ascii_uppercase()),
			modifiers: Modifiers {
				shift: true,
				..Modifiers::NONE
			},
		}
	}

	#[test]
	fn test_shift_w_uppercase_sets_extend() {
		// Terminal sends 'W' with shift=true
		// Since W has its own binding (next_long_word_start), we use that with extend
		let key = key_shifted_uppercase('w');
		assert_eq!(key.code, KeyCode::Char('W'));
		assert!(key.modifiers.shift, "terminal preserves shift modifier");

		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "next_long_word_start", "should match 'W' binding");
				assert!(extend, "shift should set extend=true");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_l_uppercase_sets_extend() {
		let key = key_shifted_uppercase('l');
		assert_eq!(key.code, KeyCode::Char('L'));
		assert!(key.modifiers.shift);

		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "move_right", "should match 'l' binding");
				assert!(extend, "shift should set extend=true");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_uppercase_w_means_long_word_not_extend() {
		// Pressing 'W' (capital) without shift modifier - this is next_long_word_start
		// In Kakoune, W is bound to WORD motion, not extend
		let mut h = InputHandler::new();
		let res = h.handle_key(Key::char('W'));
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "next_long_word_start", "W should be WORD motion");
				assert!(!extend, "no shift means no extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_u_is_redo_with_extend() {
		// Shift+U triggers U binding (redo) with extend=true
		// Redo ignores extend, but it's still set
		let key = key_shifted_uppercase('u');
		assert_eq!(key.code, KeyCode::Char('U'));
		assert!(key.modifiers.shift);

		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "redo", "Shift+U should be redo");
				assert!(extend, "shift always sets extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_w_uses_uppercase_w_binding_with_extend() {
		// W has its own binding (next_long_word_start)
		// Shift+W should use W binding with extend=true
		let key = key_shifted_uppercase('w');

		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "next_long_word_start", "Shift+W should use W binding");
				assert!(extend, "shift should set extend=true");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_page_down_extends() {
		let key = Key::special(SpecialKey::PageDown).with_shift();

		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "scroll_page_down");
				assert!(extend, "shift+pagedown should extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_page_up_extends() {
		let key = Key::special(SpecialKey::PageUp).with_shift();

		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "scroll_page_up");
				assert!(extend, "shift+pageup should extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_home_extends() {
		let key = Key::special(SpecialKey::Home).with_shift();

		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "move_line_start");
				assert!(extend, "shift+home should extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_end_extends() {
		let key = Key::special(SpecialKey::End).with_shift();

		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "move_line_end");
				assert!(extend, "shift+end should extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_page_down_no_shift_no_extend() {
		let key = Key::special(SpecialKey::PageDown);

		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "scroll_page_down");
				assert!(!extend, "pagedown without shift should not extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_arrow_extends() {
		let key = Key::special(SpecialKey::Right).with_shift();

		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "move_right");
				assert!(extend, "shift+right should extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}
}
