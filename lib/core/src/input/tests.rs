#[cfg(test)]
mod tests {
	use crate::input::{InputHandler, KeyResult};
	use crate::key::{Key, KeyCode, Modifiers, SpecialKey};

	#[test]
	fn test_digit_count_accumulates() {
		let mut h = InputHandler::new();
		h.handle_key(Key::char('2'));
		h.handle_key(Key::char('3'));
		assert_eq!(h.effective_count(), 23);
	}

	fn key_with_shift(c: char) -> Key {
		Key {
			code: KeyCode::Char(c),
			modifiers: Modifiers {
				shift: true,
				..Modifiers::NONE
			},
		}
	}

	#[test]
	fn test_word_motion_sets_extend_with_shift() {
		let mut h = InputHandler::new();
		let res = h.handle_key(key_with_shift('w'));
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "next_word_start");
				assert!(extend);
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_word_motion_no_shift_not_extend() {
		let mut h = InputHandler::new();
		let res = h.handle_key(Key::char('w'));
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "next_word_start");
				assert!(!extend);
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	/// Simulates what the terminal sends: Shift+w comes as uppercase 'W' with shift modifier.
	fn key_shifted_uppercase(c: char) -> Key {
		Key {
			code: KeyCode::Char(c.to_ascii_uppercase()),
			modifiers: Modifiers {
				shift: true,
				..Modifiers::NONE
			},
		}
	}

	#[test]
	fn test_shift_w_uppercase_sets_extend() {
		let key = key_shifted_uppercase('w');
		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "next_long_word_start", "should match 'W' binding");
				assert!(extend, "shift should set extend=true");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_l_uppercase_sets_extend() {
		let key = key_shifted_uppercase('l');
		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "move_right", "should match 'l' binding");
				assert!(extend, "shift should set extend=true");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_uppercase_w_means_long_word_not_extend() {
		let mut h = InputHandler::new();
		let res = h.handle_key(Key::char('W'));
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "next_long_word_start", "W should be WORD motion");
				assert!(!extend, "no shift means no extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_u_is_redo_with_extend() {
		let key = key_shifted_uppercase('u');
		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "redo", "Shift+U should be redo");
				assert!(extend, "shift always sets extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_w_uses_uppercase_w_binding_with_extend() {
		let key = key_shifted_uppercase('w');
		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "next_long_word_start", "Shift+W should use W binding");
				assert!(extend, "shift should set extend=true");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_page_down_extends() {
		let key = Key::special(SpecialKey::PageDown).with_shift();
		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "scroll_page_down");
				assert!(extend, "shift+pagedown should extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_page_up_extends() {
		let key = Key::special(SpecialKey::PageUp).with_shift();
		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "scroll_page_up");
				assert!(extend, "shift+pageup should extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_home_extends() {
		let key = Key::special(SpecialKey::Home).with_shift();
		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "move_line_start");
				assert!(extend, "shift+home should extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_end_extends() {
		let key = Key::special(SpecialKey::End).with_shift();
		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "move_line_end");
				assert!(extend, "shift+end should extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_page_down_no_shift_no_extend() {
		let key = Key::special(SpecialKey::PageDown);
		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "scroll_page_down");
				assert!(!extend, "pagedown without shift should not extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}

	#[test]
	fn test_shift_arrow_extends() {
		let key = Key::special(SpecialKey::Right).with_shift();
		let mut h = InputHandler::new();
		let res = h.handle_key(key);
		match res {
			KeyResult::Action { name, extend, .. } => {
				assert_eq!(name, "move_right");
				assert!(extend, "shift+right should extend");
			}
			other => panic!("unexpected result: {:?}", other),
		}
	}
}
