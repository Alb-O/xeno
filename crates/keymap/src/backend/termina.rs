//! Key event parsing and conversion for the `termina` backend.
//!
//! This module bridges `termina::event::KeyEvent` with a backend-agnostic
//! representation (`KeyMap`) used for keybinding configuration and matching.

use evildoer_keymap_parser::{self as parser, Key, Modifier, Node};
use termina::event::{KeyCode, KeyEvent, Modifiers as TmModifiers};

use crate::Error;
use crate::keymap::{FromKeyMap, IntoKeyMap, KeyMap, ToKeyMap};

/// Parses a string keybinding (e.g., `"ctrl-c"`, `"f1"`, `"alt-backspace"`) into a `KeyEvent`.
pub fn parse(s: &str) -> Result<KeyEvent, Error> {
	parser::parse(s)
		.map_err(Error::Parse)
		.and_then(KeyEvent::from_keymap)
}

impl IntoKeyMap for KeyEvent {
	fn into_keymap(self) -> Result<KeyMap, Error> {
		self.to_keymap()
	}
}

impl ToKeyMap for KeyEvent {
	fn to_keymap(&self) -> Result<KeyMap, Error> {
		let KeyEvent {
			code, modifiers, ..
		} = self;
		let key = match code {
			KeyCode::BackTab => Key::BackTab,
			KeyCode::Backspace => Key::Backspace,
			KeyCode::Char(' ') => Key::Space,
			KeyCode::Char(c) => Key::Char(*c),
			KeyCode::Delete => Key::Delete,
			KeyCode::Down => Key::Down,
			KeyCode::End => Key::End,
			KeyCode::Enter => Key::Enter,
			KeyCode::Escape => Key::Esc,
			KeyCode::Function(n) => Key::F(*n),
			KeyCode::Home => Key::Home,
			KeyCode::Insert => Key::Insert,
			KeyCode::Left => Key::Left,
			KeyCode::PageDown => Key::PageDown,
			KeyCode::PageUp => Key::PageUp,
			KeyCode::Right => Key::Right,
			KeyCode::Tab => Key::Tab,
			KeyCode::Up => Key::Up,
			code => {
				return Err(Error::UnsupportedKey(format!(
					"Unsupported KeyEvent {code:?}"
				)));
			}
		};

		Ok(Node::new(modifiers_from_backend(modifiers), key))
	}
}

impl FromKeyMap for KeyEvent {
	fn from_keymap(keymap: KeyMap) -> Result<Self, Error> {
		let key = match keymap.key {
			Key::BackTab => KeyCode::BackTab,
			Key::Backspace => KeyCode::Backspace,
			Key::Char(c) => KeyCode::Char(c),
			Key::Delete => KeyCode::Delete,
			Key::Down => KeyCode::Down,
			Key::End => KeyCode::End,
			Key::Enter => KeyCode::Enter,
			Key::Esc => KeyCode::Escape,
			Key::F(n) => KeyCode::Function(n),
			Key::Home => KeyCode::Home,
			Key::Insert => KeyCode::Insert,
			Key::Left => KeyCode::Left,
			Key::PageDown => KeyCode::PageDown,
			Key::PageUp => KeyCode::PageUp,
			Key::Right => KeyCode::Right,
			Key::Tab => KeyCode::Tab,
			Key::Space => KeyCode::Char(' '),
			Key::Up => KeyCode::Up,
			Key::Group(group) => {
				return Err(Error::UnsupportedKey(format!(
					"Group {group:?} not supported. Cannot map char group back to KeyEvent"
				)));
			}
		};

		Ok(KeyEvent::new(key, modifiers_from_node(keymap.modifiers)))
	}
}

const MODIFIERS: [(TmModifiers, parser::Modifier); 3] = [
	(TmModifiers::ALT, Modifier::Alt),
	(TmModifiers::CONTROL, Modifier::Ctrl),
	(TmModifiers::SHIFT, Modifier::Shift),
];

fn modifiers_from_backend(value: &TmModifiers) -> parser::Modifiers {
	MODIFIERS.into_iter().fold(0, |acc, (m1, m2)| {
		acc | if value.contains(m1) { m2 as u8 } else { 0 }
	})
}

fn modifiers_from_node(value: parser::Modifiers) -> TmModifiers {
	let none = TmModifiers::NONE;
	MODIFIERS.into_iter().fold(none, |acc, (m1, m2)| {
		acc | if value & (m2 as u8) != 0 { m1 } else { none }
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_simple_keys() {
		assert_eq!(
			parse("a").unwrap(),
			KeyEvent::new(KeyCode::Char('a'), TmModifiers::NONE)
		);
		assert_eq!(
			parse("esc").unwrap(),
			KeyEvent::new(KeyCode::Escape, TmModifiers::NONE)
		);
		assert_eq!(
			parse("enter").unwrap(),
			KeyEvent::new(KeyCode::Enter, TmModifiers::NONE)
		);
	}

	#[test]
	fn parse_with_modifiers() {
		assert_eq!(
			parse("ctrl-c").unwrap(),
			KeyEvent::new(KeyCode::Char('c'), TmModifiers::CONTROL)
		);
		assert_eq!(
			parse("alt-x").unwrap(),
			KeyEvent::new(KeyCode::Char('x'), TmModifiers::ALT)
		);
		assert_eq!(
			parse("shift-tab").unwrap(),
			KeyEvent::new(KeyCode::Tab, TmModifiers::SHIFT)
		);
	}

	#[test]
	fn parse_function_keys() {
		assert_eq!(
			parse("f1").unwrap(),
			KeyEvent::new(KeyCode::Function(1), TmModifiers::NONE)
		);
		assert_eq!(
			parse("ctrl-f12").unwrap(),
			KeyEvent::new(KeyCode::Function(12), TmModifiers::CONTROL)
		);
	}

	#[test]
	fn roundtrip_conversion() {
		let cases = ["a", "ctrl-b", "alt-f4", "shift-enter", "del", "space"];
		for s in cases {
			let event = parse(s).unwrap();
			let keymap = event.to_keymap().unwrap();
			let back = KeyEvent::from_keymap(keymap).unwrap();
			assert_eq!(event, back, "roundtrip failed for {s}");
		}
	}
}
