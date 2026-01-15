//! Unit tests for input handler logic.
//!
//! These tests verify internal state management that doesn't require
//! the keybinding/action registry. Integration tests that verify
//! keybinding → action resolution are in tests/keybindings.rs.

use xeno_primitives::key::Key;

use super::InputHandler;

#[test]
fn test_digit_count_accumulates() {
	let mut h = InputHandler::new();
	h.handle_key(Key::char('2'));
	h.handle_key(Key::char('3'));
	assert_eq!(h.effective_count(), 23);
}

#[test]
fn test_effective_count_defaults_to_one() {
	let h = InputHandler::new();
	assert_eq!(h.effective_count(), 1);
}

#[test]
fn test_count_resets_on_mode_change() {
	let mut h = InputHandler::new();
	h.handle_key(Key::char('5'));
	assert_eq!(h.count(), 5);

	// Reset via set_mode to Normal
	h.set_mode(super::types::Mode::Normal);
	assert_eq!(h.count(), 0);
}

#[test]
fn test_initial_mode_is_normal() {
	let h = InputHandler::new();
	assert!(matches!(h.mode(), super::types::Mode::Normal));
}

#[test]
fn test_mode_name() {
	let h = InputHandler::new();
	assert_eq!(h.mode_name(), "NORMAL");
}

#[test]
fn test_last_search_initially_none() {
	let h = InputHandler::new();
	assert!(h.last_search().is_none());
}

#[test]
fn test_set_last_search() {
	let mut h = InputHandler::new();
	h.set_last_search("pattern".to_string(), false);
	let (pattern, reverse) = h.last_search().unwrap();
	assert_eq!(pattern, "pattern");
	assert!(!reverse);

	h.set_last_search("other".to_string(), true);
	let (pattern, reverse) = h.last_search().unwrap();
	assert_eq!(pattern, "other");
	assert!(reverse);
}
