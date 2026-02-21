use xeno_keymap_core::parser::{Key as ParserKey, Modifier, Node};
use xeno_primitives::{Key, KeyCode, Modifiers};

pub(super) fn key_to_node(key: Key) -> Node {
	Node::new(modifiers_to_parser(&key.modifiers), key_to_parser(key.code))
}

fn key_to_parser(code: KeyCode) -> ParserKey {
	match code {
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
	}
}

fn modifiers_to_parser(mods: &Modifiers) -> u8 {
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
	if mods.cmd {
		result |= Modifier::Cmd as u8;
	}
	result
}
