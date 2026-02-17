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

#[test]
fn invocation_spec_multi_key_pending_then_match() {
	use std::collections::HashMap;

	use xeno_registry::config::UnresolvedKeys;
	use xeno_registry::db::keymap_registry::KeymapIndex;
	use xeno_registry::keymaps::KeymapBehavior;

	let actions = xeno_registry::db::ACTIONS.snapshot();

	// Override "g r" → editor_command reload_config
	let mut normal = HashMap::new();
	normal.insert("g r".to_string(), Some(xeno_registry::Invocation::editor_command("reload_config", vec![])));
	let mut modes = HashMap::new();
	modes.insert("normal".to_string(), normal);
	let overrides = UnresolvedKeys { modes };
	let keymap = KeymapIndex::build_with_overrides(&actions, Some(&overrides));

	let mut h = InputHandler::new();

	// First key 'g' should produce Pending
	let result = h.handle_key_with_registry(Key::char('g'), &keymap, KeymapBehavior::default());
	assert!(
		matches!(result, super::types::KeyResult::Pending { keys_so_far: 1 }),
		"expected Pending after 'g', got {result:?}"
	);

	// Second key 'r' should produce Invocation
	let result = h.handle_key_with_registry(Key::char('r'), &keymap, KeymapBehavior::default());
	match result {
		super::types::KeyResult::Invocation { ref inv } => {
			assert!(matches!(
				inv,
				xeno_registry::Invocation::Command(cmd) if cmd.name == "reload_config"
			));
		}
		_ => panic!("expected Invocation after 'g r', got {result:?}"),
	}

	// State should be reset — count should be back to default
	assert_eq!(h.effective_count(), 1);
	assert_eq!(h.pending_key_count(), 0);
}

#[test]
fn insert_multikey_prefix_dispatches() {
	use xeno_registry::db::keymap_registry::KeymapIndex;
	use xeno_registry::keymaps;

	let actions = xeno_registry::db::ACTIONS.snapshot();
	let preset = keymaps::preset("emacs").expect("emacs preset must load");
	let keymap = KeymapIndex::build_with_preset(&actions, Some(&preset), None);

	let mut h = InputHandler::new();
	h.set_mode(super::types::Mode::Insert);

	// ctrl-x should produce Pending (C-x prefix)
	let result = h.handle_key_with_registry(Key::ctrl('x'), &keymap, preset.behavior);
	assert!(
		matches!(result, super::types::KeyResult::Pending { keys_so_far: 1 }),
		"expected Pending after ctrl-x, got {result:?}"
	);

	// ctrl-s should complete the C-x C-s binding → command:write
	let result = h.handle_key_with_registry(Key::ctrl('s'), &keymap, preset.behavior);
	match result {
		super::types::KeyResult::Invocation { ref inv } => {
			assert!(
				matches!(inv, xeno_registry::Invocation::Command(cmd) if cmd.name == "write"),
				"expected command:write, got {inv:?}"
			);
		}
		_ => panic!("expected Invocation after ctrl-x ctrl-s, got {result:?}"),
	}

	assert_eq!(h.pending_key_count(), 0);
}

#[test]
fn insert_text_char_does_not_enter_pending() {
	use xeno_registry::db::keymap_registry::KeymapIndex;
	use xeno_registry::keymaps;

	let actions = xeno_registry::db::ACTIONS.snapshot();
	let preset = keymaps::preset("emacs").expect("emacs preset must load");
	let keymap = KeymapIndex::build_with_preset(&actions, Some(&preset), None);

	let mut h = InputHandler::new();
	h.set_mode(super::types::Mode::Insert);

	// Plain 'a' should insert immediately, not enter pending state
	let result = h.handle_key_with_registry(Key::char('a'), &keymap, preset.behavior);
	assert!(
		matches!(result, super::types::KeyResult::InsertChar('a')),
		"expected InsertChar('a'), got {result:?}"
	);
	assert_eq!(h.pending_key_count(), 0);
}
