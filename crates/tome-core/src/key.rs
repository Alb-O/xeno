//! Key representation for Kakoune-compatible keybindings.
//!
//! This module provides a unified key representation that handles:
//! - Regular character keys
//! - Special keys (Escape, Enter, Tab, arrows, etc.)
//! - Modifier combinations (Ctrl, Alt, Shift)

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
        Self {
            ctrl: true,
            ..self
        }
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
        if self.modifiers.is_empty() {
            if let KeyCode::Char(c) = self.code {
                return c.to_digit(10);
            }
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
        if self.modifiers.shift {
            if let KeyCode::Char(c) = self.code {
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
        if self.modifiers.shift {
            write!(f, "S-")?;
        }

        match self.code {
            KeyCode::Char(c) => write!(f, "{}", c),
            KeyCode::Special(s) => write!(f, "<{:?}>", s),
        }
    }
}

/// Conversion from crossterm's KeyEvent.
#[cfg(feature = "crossterm")]
impl From<crossterm::event::KeyEvent> for Key {
    fn from(event: crossterm::event::KeyEvent) -> Self {
        use crossterm::event::{KeyCode as CtKeyCode, KeyModifiers};

        let modifiers = Modifiers {
            ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
            alt: event.modifiers.contains(KeyModifiers::ALT),
            shift: event.modifiers.contains(KeyModifiers::SHIFT),
        };

        let code = match event.code {
            CtKeyCode::Char(c) => KeyCode::Char(c),
            CtKeyCode::Esc => KeyCode::Special(SpecialKey::Escape),
            CtKeyCode::Enter => KeyCode::Special(SpecialKey::Enter),
            CtKeyCode::Tab => KeyCode::Special(SpecialKey::Tab),
            CtKeyCode::Backspace => KeyCode::Special(SpecialKey::Backspace),
            CtKeyCode::Delete => KeyCode::Special(SpecialKey::Delete),
            CtKeyCode::Insert => KeyCode::Special(SpecialKey::Insert),
            CtKeyCode::Home => KeyCode::Special(SpecialKey::Home),
            CtKeyCode::End => KeyCode::Special(SpecialKey::End),
            CtKeyCode::PageUp => KeyCode::Special(SpecialKey::PageUp),
            CtKeyCode::PageDown => KeyCode::Special(SpecialKey::PageDown),
            CtKeyCode::Up => KeyCode::Special(SpecialKey::Up),
            CtKeyCode::Down => KeyCode::Special(SpecialKey::Down),
            CtKeyCode::Left => KeyCode::Special(SpecialKey::Left),
            CtKeyCode::Right => KeyCode::Special(SpecialKey::Right),
            CtKeyCode::F(n) => KeyCode::Special(SpecialKey::F(n)),
            _ => KeyCode::Char('\0'),
        };

        Self { code, modifiers }.normalize()
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
        assert_eq!(
            Key::special(SpecialKey::Escape).to_string(),
            "<Escape>"
        );
    }
}
