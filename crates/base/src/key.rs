//! Key representation for Kakoune-compatible keybindings.
//!
//! This module provides a unified key representation that handles:
//! - Regular character keys
//! - Special keys (Escape, Enter, Tab, arrows, etc.)
//! - Modifier combinations (Ctrl, Alt, Shift)
//! - Mouse events (clicks, drags, scrolls)

use std::fmt;

pub use evildoer_keymap_parser::Key as KeyCode;

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

/// A key with optional modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
	pub code: KeyCode,
	pub modifiers: Modifiers,
}

impl Key {
	/// Create a key from a character with no modifiers.
	pub const fn char(c: char) -> Self {
		Self {
			code: KeyCode::Char(c),
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
			modifiers: Modifiers {
				ctrl: true,
				..self.modifiers
			},
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
	Press {
		button: MouseButton,
		row: u16,
		col: u16,
		modifiers: Modifiers,
	},
	Release {
		row: u16,
		col: u16,
	},
	Drag {
		button: MouseButton,
		row: u16,
		col: u16,
		modifiers: Modifiers,
	},
	Scroll {
		direction: ScrollDirection,
		row: u16,
		col: u16,
		modifiers: Modifiers,
	},
	Move {
		row: u16,
		col: u16,
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
			MouseEvent::Press { row, .. }
			| MouseEvent::Release { row, .. }
			| MouseEvent::Drag { row, .. }
			| MouseEvent::Scroll { row, .. }
			| MouseEvent::Move { row, .. } => *row,
		}
	}

	pub fn col(&self) -> u16 {
		match self {
			MouseEvent::Press { col, .. }
			| MouseEvent::Release { col, .. }
			| MouseEvent::Drag { col, .. }
			| MouseEvent::Scroll { col, .. }
			| MouseEvent::Move { col, .. } => *col,
		}
	}

	pub fn modifiers(&self) -> Modifiers {
		match self {
			MouseEvent::Press { modifiers, .. }
			| MouseEvent::Drag { modifiers, .. }
			| MouseEvent::Scroll { modifiers, .. } => *modifiers,
			MouseEvent::Release { .. } | MouseEvent::Move { .. } => Modifiers::NONE,
		}
	}
}

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
			MouseEventKind::Moved => MouseEvent::Move {
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
		write!(f, "{}", self.code)
	}
}

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
			TmKeyCode::Escape => KeyCode::Esc,
			TmKeyCode::Enter => KeyCode::Enter,
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
			_ => KeyCode::Char('\0'),
		};

		Self { code, modifiers }
	}
}

#[cfg(test)]
mod tests;
