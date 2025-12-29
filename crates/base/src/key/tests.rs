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
	assert_eq!(Key::new(KeyCode::Esc).to_string(), "esc");
}

#[test]
fn test_special_keys() {
	assert!(Key::new(KeyCode::Esc).is_escape());
	assert!(Key::new(KeyCode::Backspace).is_backspace());
	assert!(Key::new(KeyCode::Enter).is_enter());
	assert!(Key::new(KeyCode::Tab).is_tab());
}
