//! Keyboard key types with modifier support.

use std::fmt;

use super::Modifiers;

/// Runtime keyboard key codes used by input events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
	BackTab,
	Backspace,
	Delete,
	Down,
	End,
	Enter,
	Esc,
	Home,
	Insert,
	Left,
	PageDown,
	PageUp,
	Right,
	Space,
	Tab,
	Up,
	F(u8),
	Char(char),
}

impl fmt::Display for KeyCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Char(c) => write!(f, "{c}"),
			Self::F(n) => write!(f, "f{n}"),
			Self::BackTab => write!(f, "backtab"),
			Self::Backspace => write!(f, "backspace"),
			Self::Delete => write!(f, "delete"),
			Self::Down => write!(f, "down"),
			Self::End => write!(f, "end"),
			Self::Enter => write!(f, "enter"),
			Self::Esc => write!(f, "esc"),
			Self::Home => write!(f, "home"),
			Self::Insert => write!(f, "insert"),
			Self::Left => write!(f, "left"),
			Self::PageDown => write!(f, "pagedown"),
			Self::PageUp => write!(f, "pageup"),
			Self::Right => write!(f, "right"),
			Self::Space => write!(f, "space"),
			Self::Tab => write!(f, "tab"),
			Self::Up => write!(f, "up"),
		}
	}
}

/// A key with optional modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
	/// The key code (character, special key, or function key).
	pub code: KeyCode,
	/// Active modifiers for this key event.
	pub modifiers: Modifiers,
}

impl Key {
	/// Create a key from a character with no modifiers.
	///
	/// Whitespace characters are canonicalized: `' '` → `Space`, `'\t'` → `Tab`,
	/// `'\n'` → `Enter`.
	pub const fn char(c: char) -> Self {
		let code = match c {
			' ' => KeyCode::Space,
			'\t' => KeyCode::Tab,
			'\n' => KeyCode::Enter,
			_ => KeyCode::Char(c),
		};
		Self {
			code,
			modifiers: Modifiers::NONE,
		}
	}

	/// Create a key from a key code with no modifiers.
	pub const fn new(code: KeyCode) -> Self {
		Self {
			code,
			modifiers: Modifiers::NONE,
		}
	}

	/// Create a key with Ctrl modifier.
	pub const fn ctrl(c: char) -> Self {
		Self {
			code: KeyCode::Char(c),
			modifiers: Modifiers::CTRL,
		}
	}

	/// Create a key with Alt modifier.
	pub const fn alt(c: char) -> Self {
		Self {
			code: KeyCode::Char(c),
			modifiers: Modifiers::ALT,
		}
	}

	/// Add Ctrl modifier.
	pub const fn with_ctrl(self) -> Self {
		Self {
			modifiers: Modifiers { ctrl: true, ..self.modifiers },
			..self
		}
	}

	/// Drop the shift modifier (useful for treating Shift as "extend"), preserving codepoint.
	pub const fn drop_shift(self) -> Self {
		Self {
			modifiers: Modifiers {
				shift: false,
				..self.modifiers
			},
			..self
		}
	}

	/// Add Alt modifier.
	pub const fn with_alt(self) -> Self {
		Self {
			modifiers: Modifiers { alt: true, ..self.modifiers },
			..self
		}
	}

	/// Add Shift modifier.
	pub const fn with_shift(self) -> Self {
		Self {
			modifiers: Modifiers { shift: true, ..self.modifiers },
			..self
		}
	}

	/// Canonicalizes the key code so equivalent inputs have one representation.
	///
	/// `Char(' ')` → `Space`, `Char('\t')` → `Tab`, `Char('\n')` → `Enter`.
	pub fn canonicalize(self) -> Self {
		let code = match self.code {
			KeyCode::Char(' ') => KeyCode::Space,
			KeyCode::Char('\t') => KeyCode::Tab,
			KeyCode::Char('\n') => KeyCode::Enter,
			other => other,
		};
		Self { code, ..self }
	}

	/// Check if this is a digit key (for count prefixes).
	pub fn as_digit(&self) -> Option<u32> {
		if self.modifiers.is_empty()
			&& let KeyCode::Char(c) = self.code
		{
			return c.to_digit(10);
		}
		None
	}

	/// Check if this is a specific character (ignoring modifiers).
	pub fn is_char(&self, c: char) -> bool {
		matches!(self.code, KeyCode::Char(ch) if ch == c)
	}

	/// Get the character if this is a character key.
	pub fn codepoint(&self) -> Option<char> {
		match self.code {
			KeyCode::Char(c) => Some(c),
			KeyCode::Space => Some(' '),
			_ => None,
		}
	}

	/// Check if this key is escape.
	pub fn is_escape(&self) -> bool {
		matches!(self.code, KeyCode::Esc) && self.modifiers.is_empty()
	}

	/// Check if this key is backspace.
	pub fn is_backspace(&self) -> bool {
		matches!(self.code, KeyCode::Backspace) && self.modifiers.is_empty()
	}

	/// Check if this key is delete.
	pub fn is_delete(&self) -> bool {
		matches!(self.code, KeyCode::Delete) && self.modifiers.is_empty()
	}

	/// Check if this key is enter.
	pub fn is_enter(&self) -> bool {
		matches!(self.code, KeyCode::Enter) && self.modifiers.is_empty()
	}

	/// Check if this key is tab.
	pub fn is_tab(&self) -> bool {
		matches!(self.code, KeyCode::Tab) && self.modifiers.is_empty()
	}

	/// Convert a shifted letter to uppercase for matching.
	/// e.g., Shift+h -> H, Shift+U -> U (drop shift for uppercase letters)
	pub fn normalize(self) -> Self {
		if self.modifiers.shift
			&& let KeyCode::Char(c) = self.code
		{
			if c.is_ascii_lowercase() {
				return Self {
					code: KeyCode::Char(c.to_ascii_uppercase()),
					modifiers: Modifiers {
						shift: false,
						..self.modifiers
					},
				};
			} else if c.is_ascii_uppercase() {
				return Self {
					code: KeyCode::Char(c),
					modifiers: Modifiers {
						shift: false,
						..self.modifiers
					},
				};
			}
		}
		self
	}
}

impl fmt::Display for Key {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.modifiers.ctrl {
			write!(f, "C-")?;
		}
		if self.modifiers.alt {
			write!(f, "A-")?;
		}
		if self.modifiers.cmd {
			write!(f, "D-")?;
		}
		if self.modifiers.shift {
			write!(f, "S-")?;
		}
		write!(f, "{}", self.code)
	}
}

#[cfg(feature = "terminal-input")]
impl TryFrom<termina::event::KeyEvent> for Key {
	type Error = ();

	fn try_from(event: termina::event::KeyEvent) -> Result<Self, ()> {
		use termina::event::{KeyCode as TmKeyCode, Modifiers as TmModifiers};

		let modifiers = Modifiers {
			ctrl: event.modifiers.contains(TmModifiers::CONTROL),
			alt: event.modifiers.contains(TmModifiers::ALT),
			shift: event.modifiers.contains(TmModifiers::SHIFT),
			cmd: event.modifiers.contains(TmModifiers::SUPER),
		};

		let code = match event.code {
			TmKeyCode::Char(c) => KeyCode::Char(c),
			TmKeyCode::Escape => KeyCode::Esc,
			TmKeyCode::Enter => KeyCode::Enter,
			TmKeyCode::BackTab => KeyCode::BackTab,
			TmKeyCode::Tab => KeyCode::Tab,
			TmKeyCode::Backspace => KeyCode::Backspace,
			TmKeyCode::Delete => KeyCode::Delete,
			TmKeyCode::Insert => KeyCode::Insert,
			TmKeyCode::Home => KeyCode::Home,
			TmKeyCode::End => KeyCode::End,
			TmKeyCode::PageUp => KeyCode::PageUp,
			TmKeyCode::PageDown => KeyCode::PageDown,
			TmKeyCode::Up => KeyCode::Up,
			TmKeyCode::Down => KeyCode::Down,
			TmKeyCode::Left => KeyCode::Left,
			TmKeyCode::Right => KeyCode::Right,
			TmKeyCode::Function(n) => KeyCode::F(n),
			_ => return Err(()),
		};

		Ok(Self { code, modifiers }.canonicalize())
	}
}

#[cfg(all(test, feature = "terminal-input"))]
mod termina_tests {
	use termina::event::{KeyCode as TmKeyCode, KeyEvent, KeyEventKind, KeyEventState, Modifiers as TmModifiers};

	use super::*;

	fn key_event(code: TmKeyCode, modifiers: TmModifiers) -> KeyEvent {
		KeyEvent {
			code,
			modifiers,
			kind: KeyEventKind::Press,
			state: KeyEventState::NONE,
		}
	}

	#[test]
	fn unknown_keycode_returns_err() {
		let event = key_event(TmKeyCode::Null, TmModifiers::empty());
		assert!(Key::try_from(event).is_err());
	}

	#[test]
	fn space_char_canonicalizes_to_space_keycode() {
		let event = key_event(TmKeyCode::Char(' '), TmModifiers::empty());
		let key = Key::try_from(event).unwrap();
		assert_eq!(key.code, KeyCode::Space);
	}

	#[test]
	fn super_modifier_maps_to_cmd() {
		let event = key_event(TmKeyCode::Char('a'), TmModifiers::SUPER);
		let key = Key::try_from(event).unwrap();
		assert!(key.modifiers.cmd);
		assert!(!key.modifiers.ctrl);
	}

	#[test]
	fn function_key_35_maps_correctly() {
		let event = key_event(TmKeyCode::Function(35), TmModifiers::empty());
		let key = Key::try_from(event).unwrap();
		assert_eq!(key.code, KeyCode::F(35));
	}
}
