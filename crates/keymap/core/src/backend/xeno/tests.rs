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
