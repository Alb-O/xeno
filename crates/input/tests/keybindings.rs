//! Integration tests for input handling with keybindings.
//!
//! These tests require the full registry (keybindings + actions) to be linked,
//! which happens automatically in integration tests since they link all dependencies.

// Force linkage of xeno-stdlib to ensure all actions are registered.
// This is necessary because linkme distributed slices only include statics
// that are actually linked into the binary.
extern crate xeno_core;

use xeno_primitives::key::{Key, KeyCode, Modifiers};
use xeno_core::find_action_by_id;
use xeno_input::{InputHandler, KeyResult, Mode};

fn extract_action(result: KeyResult) -> Option<(String, bool)> {
	match result {
		KeyResult::ActionById { id, extend, .. } => {
			find_action_by_id(id).map(|def| (def.name.to_string(), extend))
		}
		_ => None,
	}
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

/// Simulates what the terminal sends: Shift+letter comes as uppercase with shift modifier.
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
fn test_word_motion_sets_extend_with_shift() {
	let mut h = InputHandler::new();
	let res = h.handle_key(key_with_shift('w'));
	let (name, extend) = extract_action(res).expect("should return an action for shift+w");
	assert_eq!(name, "next_word_start");
	assert!(extend);
}

#[test]
fn test_word_motion_no_shift_not_extend() {
	let mut h = InputHandler::new();
	let res = h.handle_key(Key::char('w'));
	let (name, extend) = extract_action(res).expect("should return an action for w");
	assert_eq!(name, "next_word_start");
	assert!(!extend);
}

#[test]
fn test_shift_w_uppercase_sets_extend() {
	let key = key_shifted_uppercase('w');
	let mut h = InputHandler::new();
	let res = h.handle_key(key);
	let (name, extend) = extract_action(res).expect("should return an action for Shift+W");
	assert_eq!(name, "next_long_word_start", "should match 'W' binding");
	assert!(extend, "shift should set extend=true");
}

#[test]
fn test_shift_l_uppercase_sets_extend() {
	let key = key_shifted_uppercase('l');
	let mut h = InputHandler::new();
	let res = h.handle_key(key);
	let (name, extend) = extract_action(res).expect("should return an action for Shift+L");
	assert_eq!(name, "move_right", "should match 'l' binding");
	assert!(extend, "shift should set extend=true");
}

#[test]
fn test_uppercase_w_means_long_word_not_extend() {
	let mut h = InputHandler::new();
	let res = h.handle_key(Key::char('W'));
	let (name, extend) = extract_action(res).expect("should return an action for W");
	assert_eq!(name, "next_long_word_start", "W should be WORD motion");
	assert!(!extend, "no shift means no extend");
}

#[test]
fn test_shift_u_is_redo_with_extend() {
	let key = key_shifted_uppercase('u');
	let mut h = InputHandler::new();
	let res = h.handle_key(key);
	let (name, extend) = extract_action(res).expect("should return an action for Shift+U");
	assert_eq!(name, "redo", "Shift+U should be redo");
	assert!(extend, "shift always sets extend");
}

#[test]
fn test_shift_w_uses_uppercase_w_binding_with_extend() {
	let key = key_shifted_uppercase('w');
	let mut h = InputHandler::new();
	let res = h.handle_key(key);
	let (name, extend) = extract_action(res).expect("should return an action for Shift+W");
	assert_eq!(name, "next_long_word_start", "Shift+W should use W binding");
	assert!(extend, "shift should set extend=true");
}

#[test]
fn test_shift_page_down_extends() {
	let key = Key::new(KeyCode::PageDown).with_shift();
	let mut h = InputHandler::new();
	let res = h.handle_key(key);
	let (name, extend) = extract_action(res).expect("should return an action for Shift+PageDown");
	assert_eq!(name, "scroll_page_down");
	assert!(extend, "shift+pagedown should extend");
}

#[test]
fn test_shift_page_up_extends() {
	let key = Key::new(KeyCode::PageUp).with_shift();
	let mut h = InputHandler::new();
	let res = h.handle_key(key);
	let (name, extend) = extract_action(res).expect("should return an action for Shift+PageUp");
	assert_eq!(name, "scroll_page_up");
	assert!(extend, "shift+pageup should extend");
}

#[test]
fn test_shift_home_extends() {
	let key = Key::new(KeyCode::Home).with_shift();
	let mut h = InputHandler::new();
	let res = h.handle_key(key);
	let (name, extend) = extract_action(res).expect("should return an action for Shift+Home");
	assert_eq!(name, "move_line_start");
	assert!(extend, "shift+home should extend");
}

#[test]
fn test_shift_end_extends() {
	let key = Key::new(KeyCode::End).with_shift();
	let mut h = InputHandler::new();
	let res = h.handle_key(key);
	let (name, extend) = extract_action(res).expect("should return an action for Shift+End");
	assert_eq!(name, "move_line_end");
	assert!(extend, "shift+end should extend");
}

#[test]
fn test_page_down_no_shift_no_extend() {
	let key = Key::new(KeyCode::PageDown);
	let mut h = InputHandler::new();
	let res = h.handle_key(key);
	let (name, extend) = extract_action(res).expect("should return an action for PageDown");
	assert_eq!(name, "scroll_page_down");
	assert!(!extend, "pagedown without shift should not extend");
}

#[test]
fn test_shift_arrow_extends() {
	let key = Key::new(KeyCode::Right).with_shift();
	let mut h = InputHandler::new();
	let res = h.handle_key(key);
	let (name, extend) = extract_action(res).expect("should return an action for Shift+Right");
	assert_eq!(name, "move_right");
	assert!(extend, "shift+right should extend");
}

// Insert mode shift+selection tests

#[test]
fn test_insert_mode_shift_arrow_extends() {
	let mut h = InputHandler::new();
	h.set_mode(Mode::Insert);
	let key = Key::new(KeyCode::Right).with_shift();
	let res = h.handle_key(key);
	let (name, extend) =
		extract_action(res).expect("should return an action for Shift+Right in insert mode");
	assert_eq!(name, "move_right");
	assert!(extend, "shift+right in insert mode should extend");
}

#[test]
fn test_insert_mode_shift_page_down_extends() {
	let mut h = InputHandler::new();
	h.set_mode(Mode::Insert);
	let key = Key::new(KeyCode::PageDown).with_shift();
	let res = h.handle_key(key);
	let (name, extend) =
		extract_action(res).expect("should return an action for Shift+PageDown in insert mode");
	assert_eq!(name, "scroll_page_down");
	assert!(extend, "shift+pagedown in insert mode should extend");
}

#[test]
fn test_insert_mode_arrow_no_shift_no_extend() {
	let mut h = InputHandler::new();
	h.set_mode(Mode::Insert);
	let key = Key::new(KeyCode::Right);
	let res = h.handle_key(key);
	let (name, extend) =
		extract_action(res).expect("should return an action for Right in insert mode");
	assert_eq!(name, "move_right");
	assert!(
		!extend,
		"right without shift in insert mode should not extend"
	);
}

#[test]
fn test_insert_mode_shift_preserves_uppercase_typing() {
	let mut h = InputHandler::new();
	h.set_mode(Mode::Insert);
	// Shift+A should type uppercase 'A', not trigger selection extend
	let key = key_shifted_uppercase('a');
	let res = h.handle_key(key);
	match res {
		KeyResult::InsertChar(c) => assert_eq!(c, 'A', "Shift+A should insert uppercase A"),
		other => panic!("expected InsertChar('A'), got {:?}", other),
	}
}
