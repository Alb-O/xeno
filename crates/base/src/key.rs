//! Key representation for Kakoune-compatible keybindings.
//!
//! This module provides a unified key representation that handles:
//! - Regular character keys
//! - Special keys (Escape, Enter, Tab, arrows, etc.)
//! - Modifier combinations (Ctrl, Alt, Shift)
//! - Mouse events (clicks, drags, scrolls)

use std::fmt;

/// Key modifiers (Ctrl, Alt, Shift).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
	pub ctrl: bool,
	pub alt: bool,
	pub shift: bool,
}

impl Modifiers {
	pub const NONE: Self = Self {
		ctrl: false,
		alt: false,
		shift: false,
	};

	pub const CTRL: Self = Self {
		ctrl: true,
		alt: false,
		shift: false,
	};

	pub const ALT: Self = Self {
		ctrl: false,
		alt: true,
		shift: false,
	};

	pub const SHIFT: Self = Self {
		ctrl: false,
		alt: false,
		shift: true,
	};

	pub fn ctrl(self) -> Self {
		Self { ctrl: true, ..self }
	}

	pub fn alt(self) -> Self {
		Self { alt: true, ..self }
	}

	pub fn shift(self) -> Self {
		Self {
			shift: true,
			..self
		}
	}

	pub fn is_empty(self) -> bool {
		!self.ctrl && !self.alt && !self.shift
	}
}

/// Special (non-character) keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialKey {
	Escape,
	Enter,
	Tab,
	Backspace,
	Delete,
	Insert,
	Home,
	End,
	PageUp,
	PageDown,
	Up,
	Down,
	Left,
	Right,
	F(u8),
}

/// A key with optional modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
	pub code: KeyCode,
	pub modifiers: Modifiers,
}

/// The key code itself - either a character or a special key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
	Char(char),
	Special(SpecialKey),
}

impl Key {
	/// Create a key from a character with no modifiers.
	pub const fn char(c: char) -> Self {
		Self {
			code: KeyCode::Char(c),
			modifiers: Modifiers::NONE,
		}
	}

	/// Create a key from a special key with no modifiers.
	pub const fn special(key: SpecialKey) -> Self {
		Self {
			code: KeyCode::Special(key),
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

	/// Create a key with Shift modifier (for special keys).
	pub const fn shift(key: SpecialKey) -> Self {
		Self {
			code: KeyCode::Special(key),
			modifiers: Modifiers::SHIFT,
		}
	}

	/// Add Ctrl modifier.
	pub const fn with_ctrl(self) -> Self {
		Self {
			modifiers: Modifiers {
				ctrl: true,
				..self.modifiers
			},
			..self
		}
	}

	/// Drop the shift modifier (useful for treating Shift as “extend”), preserving codepoint.
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
			modifiers: Modifiers {
				alt: true,
				..self.modifiers
			},
			..self
		}
	}

	/// Add Shift modifier.
	pub const fn with_shift(self) -> Self {
		Self {
			modifiers: Modifiers {
				shift: true,
				..self.modifiers
			},
			..self
		}
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
			KeyCode::Special(_) => None,
		}
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
				// Already uppercase, just drop the shift modifier
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

/// Mouse button types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
	Left,
	Right,
	Middle,
}

/// Mouse event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEvent {
	/// Mouse button pressed at position.
	Press {
		button: MouseButton,
		row: u16,
		col: u16,
		modifiers: Modifiers,
	},
	/// Mouse released at position.
	Release { row: u16, col: u16 },
	/// Mouse dragged to position (button held).
	Drag {
		button: MouseButton,
		row: u16,
		col: u16,
		modifiers: Modifiers,
	},
	/// Scroll wheel moved.
	Scroll {
		direction: ScrollDirection,
		row: u16,
		col: u16,
		modifiers: Modifiers,
	},
}

/// Scroll direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrollDirection {
	Up,
	Down,
	Left,
	Right,
}

impl MouseEvent {
	pub fn row(&self) -> u16 {
		match self {
			MouseEvent::Press { row, .. } => *row,
			MouseEvent::Release { row, .. } => *row,
			MouseEvent::Drag { row, .. } => *row,
			MouseEvent::Scroll { row, .. } => *row,
		}
	}

	pub fn col(&self) -> u16 {
		match self {
			MouseEvent::Press { col, .. } => *col,
			MouseEvent::Release { col, .. } => *col,
			MouseEvent::Drag { col, .. } => *col,
			MouseEvent::Scroll { col, .. } => *col,
		}
	}

	pub fn modifiers(&self) -> Modifiers {
		match self {
			MouseEvent::Press { modifiers, .. } => *modifiers,
			MouseEvent::Release { .. } => Modifiers::NONE,
			MouseEvent::Drag { modifiers, .. } => *modifiers,
			MouseEvent::Scroll { modifiers, .. } => *modifiers,
		}
	}
}

/// Conversion from termina's MouseEvent.
impl From<termina::event::MouseEvent> for MouseEvent {
	fn from(event: termina::event::MouseEvent) -> Self {
		use termina::event::{Modifiers as TmModifiers, MouseButton as TmButton, MouseEventKind};

		let modifiers = Modifiers {
			ctrl: event.modifiers.contains(TmModifiers::CONTROL),
			alt: event.modifiers.contains(TmModifiers::ALT),
			shift: event.modifiers.contains(TmModifiers::SHIFT),
		};

		let convert_button = |btn: TmButton| match btn {
			TmButton::Left => MouseButton::Left,
			TmButton::Right => MouseButton::Right,
			TmButton::Middle => MouseButton::Middle,
		};

		match event.kind {
			MouseEventKind::Down(btn) => MouseEvent::Press {
				button: convert_button(btn),
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::Up(_) => MouseEvent::Release {
				row: event.row,
				col: event.column,
			},
			MouseEventKind::Drag(btn) => MouseEvent::Drag {
				button: convert_button(btn),
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::ScrollUp => MouseEvent::Scroll {
				direction: ScrollDirection::Up,
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::ScrollDown => MouseEvent::Scroll {
				direction: ScrollDirection::Down,
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::ScrollLeft => MouseEvent::Scroll {
				direction: ScrollDirection::Left,
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::ScrollRight => MouseEvent::Scroll {
				direction: ScrollDirection::Right,
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::Moved => MouseEvent::Release {
				row: event.row,
				col: event.column,
			},
		}
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
		if self.modifiers.shift {
			write!(f, "S-")?;
		}

		match self.code {
			KeyCode::Char(c) => write!(f, "{}", c),
			KeyCode::Special(s) => write!(f, "<{:?}>", s),
		}
	}
}

/// Conversion from termina's KeyEvent.
impl From<termina::event::KeyEvent> for Key {
	fn from(event: termina::event::KeyEvent) -> Self {
		use termina::event::{KeyCode as TmKeyCode, Modifiers as TmModifiers};

		let modifiers = Modifiers {
			ctrl: event.modifiers.contains(TmModifiers::CONTROL),
			alt: event.modifiers.contains(TmModifiers::ALT),
			shift: event.modifiers.contains(TmModifiers::SHIFT),
		};

		let code = match event.code {
			TmKeyCode::Char(c) => KeyCode::Char(c),
			TmKeyCode::Escape => KeyCode::Special(SpecialKey::Escape),
			TmKeyCode::Enter => KeyCode::Special(SpecialKey::Enter),
			TmKeyCode::Tab => KeyCode::Special(SpecialKey::Tab),
			TmKeyCode::Backspace => KeyCode::Special(SpecialKey::Backspace),
			TmKeyCode::Delete => KeyCode::Special(SpecialKey::Delete),
			TmKeyCode::Insert => KeyCode::Special(SpecialKey::Insert),
			TmKeyCode::Home => KeyCode::Special(SpecialKey::Home),
			TmKeyCode::End => KeyCode::Special(SpecialKey::End),
			TmKeyCode::PageUp => KeyCode::Special(SpecialKey::PageUp),
			TmKeyCode::PageDown => KeyCode::Special(SpecialKey::PageDown),
			TmKeyCode::Up => KeyCode::Special(SpecialKey::Up),
			TmKeyCode::Down => KeyCode::Special(SpecialKey::Down),
			TmKeyCode::Left => KeyCode::Special(SpecialKey::Left),
			TmKeyCode::Right => KeyCode::Special(SpecialKey::Right),
			TmKeyCode::Function(n) => KeyCode::Special(SpecialKey::F(n)),
			_ => KeyCode::Char('\0'),
		};

		Self { code, modifiers }
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_char_key() {
		let key = Key::char('h');
		assert!(key.is_char('h'));
		assert_eq!(key.codepoint(), Some('h'));
		assert!(key.modifiers.is_empty());
	}

	#[test]
	fn test_ctrl_key() {
		let key = Key::ctrl('c');
		assert!(key.is_char('c'));
		assert!(key.modifiers.ctrl);
		assert!(!key.modifiers.alt);
	}

	#[test]
	fn test_alt_key() {
		let key = Key::alt('w');
		assert!(key.is_char('w'));
		assert!(key.modifiers.alt);
		assert!(!key.modifiers.ctrl);
	}

	#[test]
	fn test_digit() {
		assert_eq!(Key::char('5').as_digit(), Some(5));
		assert_eq!(Key::char('0').as_digit(), Some(0));
		assert_eq!(Key::char('a').as_digit(), None);
		assert_eq!(Key::ctrl('5').as_digit(), None);
	}

	#[test]
	fn test_normalize() {
		let shifted = Key {
			code: KeyCode::Char('h'),
			modifiers: Modifiers::SHIFT,
		};
		let normalized = shifted.normalize();
		assert!(normalized.is_char('H'));
		assert!(!normalized.modifiers.shift);
	}

	#[test]
	fn test_display() {
		assert_eq!(Key::char('h').to_string(), "h");
		assert_eq!(Key::ctrl('c').to_string(), "C-c");
		assert_eq!(Key::alt('w').to_string(), "A-w");
		assert_eq!(Key::special(SpecialKey::Escape).to_string(), "<Escape>");
	}
}
