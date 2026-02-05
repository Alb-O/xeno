//! Key event conversion for `xeno-primitives` key types.
//!
//! Bridges `xeno_primitives::key::Key` with the `KeyMap` (`Node`) representation
//! used for keybinding configuration and matching.

use xeno_keymap_parser::{self as parser, Key as ParserKey, Modifier, Node};
use xeno_primitives::key::{Key, KeyCode, Modifiers};

use crate::Error;
use crate::keymap::{KeyMap, ToKeyMap};

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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn simple_char_key() {
		let key = Key::char('a');
		let node = key.to_keymap().unwrap();
		assert_eq!(node.key, ParserKey::Char('a'));
		assert_eq!(node.modifiers, 0);
	}

	#[test]
	fn key_with_modifiers() {
		let key = Key::ctrl('c');
		let node = key.to_keymap().unwrap();
		assert_eq!(node.key, ParserKey::Char('c'));
		assert_ne!(node.modifiers & (Modifier::Ctrl as u8), 0);
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
		}
	}

	#[test]
	fn alt_shift_combo() {
		let key = Key::alt('x').with_shift();
		let node = key.to_keymap().unwrap();

		assert_ne!(node.modifiers & (Modifier::Alt as u8), 0);
		assert_ne!(node.modifiers & (Modifier::Shift as u8), 0);
		assert_eq!(node.modifiers & (Modifier::Ctrl as u8), 0);
	}
}
