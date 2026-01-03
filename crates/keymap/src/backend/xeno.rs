//! Key event conversion for `xeno-base` key types.
//!
//! Bridges `xeno_base::key::Key` with the `KeyMap` (`Node`) representation
//! used for keybinding configuration and matching.

use xeno_base::key::{Key, KeyCode, Modifiers};
use xeno_keymap_parser::{self as parser, Key as ParserKey, Modifier, Node};

use crate::Error;
use crate::keymap::{FromKeyMap, IntoKeyMap, KeyMap, ToKeyMap};

impl IntoKeyMap for Key {
	fn into_keymap(self) -> Result<KeyMap, Error> {
		self.to_keymap()
	}
}

impl ToKeyMap for Key {
	fn to_keymap(&self) -> Result<KeyMap, Error> {
		let key = match self.code {
			KeyCode::BackTab => ParserKey::BackTab,
			KeyCode::Backspace => ParserKey::Backspace,
			KeyCode::Char(' ') => ParserKey::Space,
			KeyCode::Char(c) => ParserKey::Char(c),
			KeyCode::Delete => ParserKey::Delete,
			KeyCode::Down => ParserKey::Down,
			KeyCode::End => ParserKey::End,
			KeyCode::Enter => ParserKey::Enter,
			KeyCode::Esc => ParserKey::Esc,
			KeyCode::F(n) => ParserKey::F(n),
			KeyCode::Home => ParserKey::Home,
			KeyCode::Insert => ParserKey::Insert,
			KeyCode::Left => ParserKey::Left,
			KeyCode::PageDown => ParserKey::PageDown,
			KeyCode::PageUp => ParserKey::PageUp,
			KeyCode::Right => ParserKey::Right,
			KeyCode::Space => ParserKey::Space,
			KeyCode::Tab => ParserKey::Tab,
			KeyCode::Up => ParserKey::Up,
			KeyCode::Group(group) => ParserKey::Group(group),
		};

		Ok(Node::new(modifiers_to_parser(&self.modifiers), key))
	}
}

impl FromKeyMap for Key {
	fn from_keymap(keymap: KeyMap) -> Result<Self, Error> {
		let code = match keymap.key {
			ParserKey::BackTab => KeyCode::BackTab,
			ParserKey::Backspace => KeyCode::Backspace,
			ParserKey::Char(c) => KeyCode::Char(c),
			ParserKey::Delete => KeyCode::Delete,
			ParserKey::Down => KeyCode::Down,
			ParserKey::End => KeyCode::End,
			ParserKey::Enter => KeyCode::Enter,
			ParserKey::Esc => KeyCode::Esc,
			ParserKey::F(n) => KeyCode::F(n),
			ParserKey::Home => KeyCode::Home,
			ParserKey::Insert => KeyCode::Insert,
			ParserKey::Left => KeyCode::Left,
			ParserKey::PageDown => KeyCode::PageDown,
			ParserKey::PageUp => KeyCode::PageUp,
			ParserKey::Right => KeyCode::Right,
			ParserKey::Space => KeyCode::Space,
			ParserKey::Tab => KeyCode::Tab,
			ParserKey::Up => KeyCode::Up,
			ParserKey::Group(group) => {
				return Err(Error::UnsupportedKey(format!(
					"Group {group:?} cannot be converted to a concrete Key"
				)));
			}
		};

		Ok(Key {
			code,
			modifiers: modifiers_from_parser(keymap.modifiers),
		})
	}
}

/// Converts xeno Modifiers to parser bitflags.
fn modifiers_to_parser(mods: &Modifiers) -> parser::Modifiers {
	let mut result: u8 = 0;
	if mods.ctrl {
		result |= Modifier::Ctrl as u8;
	}
	if mods.alt {
		result |= Modifier::Alt as u8;
	}
	if mods.shift {
		result |= Modifier::Shift as u8;
	}
	result
}

/// Converts parser bitflags to xeno Modifiers.
fn modifiers_from_parser(mods: parser::Modifiers) -> Modifiers {
	Modifiers {
		ctrl: mods & (Modifier::Ctrl as u8) != 0,
		alt: mods & (Modifier::Alt as u8) != 0,
		shift: mods & (Modifier::Shift as u8) != 0,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn simple_char_key() {
		let key = Key::char('a');
		let node = key.to_keymap().unwrap();
		assert_eq!(node.key, ParserKey::Char('a'));
		assert_eq!(node.modifiers, 0);

		let back = Key::from_keymap(node).unwrap();
		assert_eq!(back, key);
	}

	#[test]
	fn key_with_modifiers() {
		let key = Key::ctrl('c');
		let node = key.to_keymap().unwrap();
		assert_eq!(node.key, ParserKey::Char('c'));
		assert_ne!(node.modifiers & (Modifier::Ctrl as u8), 0);

		let back = Key::from_keymap(node).unwrap();
		assert_eq!(back, key);
	}

	#[test]
	fn special_keys() {
		for (key, expected) in [
			(Key::new(KeyCode::Esc), ParserKey::Esc),
			(Key::new(KeyCode::Enter), ParserKey::Enter),
			(Key::new(KeyCode::Tab), ParserKey::Tab),
			(Key::new(KeyCode::Backspace), ParserKey::Backspace),
			(Key::new(KeyCode::F(1)), ParserKey::F(1)),
		] {
			let node = key.to_keymap().unwrap();
			assert_eq!(node.key, expected);

			let back = Key::from_keymap(node).unwrap();
			assert_eq!(back, key);
		}
	}

	#[test]
	fn alt_shift_combo() {
		let key = Key::alt('x').with_shift();
		let node = key.to_keymap().unwrap();

		assert_ne!(node.modifiers & (Modifier::Alt as u8), 0);
		assert_ne!(node.modifiers & (Modifier::Shift as u8), 0);
		assert_eq!(node.modifiers & (Modifier::Ctrl as u8), 0);

		let back = Key::from_keymap(node).unwrap();
		assert_eq!(back, key);
	}
}
