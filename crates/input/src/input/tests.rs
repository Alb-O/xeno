//! Unit tests for input handler logic.
//!
//! These tests verify internal state management that doesn't require
//! the keybinding/action registry. Integration tests that verify
//! keybinding → action resolution are in tests/keybindings.rs.

use xeno_primitives::Key;

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

	use xeno_registry::KeymapSnapshot;
	use xeno_registry::config::UnresolvedKeys;
	use xeno_registry::keymaps::KeymapBehavior;

	let actions = xeno_registry::ACTIONS.snapshot();

	// Override "g r" → editor_command reload_config
	let mut normal = HashMap::new();
	normal.insert("g r".to_string(), Some(xeno_registry::Invocation::editor_command("reload_config", vec![])));
	let mut modes = HashMap::new();
	modes.insert("normal".to_string(), normal);
	let overrides = UnresolvedKeys { modes };
	let keymap = KeymapSnapshot::build_with_overrides(&actions, Some(&overrides));

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
		super::types::KeyResult::Dispatch(super::types::KeyDispatch { ref invocation }) => {
			assert!(matches!(
				invocation,
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
	use xeno_registry::{KeymapSnapshot, keymaps};

	let actions = xeno_registry::ACTIONS.snapshot();
	let preset = keymaps::preset("emacs").expect("emacs preset must load");
	let keymap = KeymapSnapshot::build_with_preset(&actions, Some(&preset), None);

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
		super::types::KeyResult::Dispatch(super::types::KeyDispatch { ref invocation }) => {
			assert!(
				matches!(invocation, xeno_registry::Invocation::Command(cmd) if cmd.name == "write"),
				"expected command:write, got {invocation:?}"
			);
		}
		_ => panic!("expected Invocation after ctrl-x ctrl-s, got {result:?}"),
	}

	assert_eq!(h.pending_key_count(), 0);
}

#[test]
fn insert_text_char_does_not_enter_pending() {
	use xeno_registry::{KeymapSnapshot, keymaps};

	let actions = xeno_registry::ACTIONS.snapshot();
	let preset = keymaps::preset("emacs").expect("emacs preset must load");
	let keymap = KeymapSnapshot::build_with_preset(&actions, Some(&preset), None);

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

#[test]
fn action_count_overflow_clamped_to_max() {
	use xeno_registry::MAX_ACTION_COUNT;

	let binding = xeno_registry::test_support::action_binding("test_action", usize::MAX, false, None);

	let mut h = InputHandler::new();
	h.count = u32::MAX;

	let result = h.consume_binding(&binding);
	match result {
		super::types::KeyResult::Dispatch(super::types::KeyDispatch { invocation }) => match invocation {
			xeno_registry::Invocation::Action { count, .. } => {
				assert!(count <= MAX_ACTION_COUNT, "count {count} exceeds MAX_ACTION_COUNT {MAX_ACTION_COUNT}");
			}
			other => panic!("expected Action invocation, got {other:?}"),
		},
		other => panic!("expected Dispatch, got {other:?}"),
	}
}

/// Golden table: `key_to_node(Key)` must produce the same `Node` as `parse(keymap_string)`.
///
/// This ensures the runtime key representation and the keymap parser agree
/// on canonical form, preventing silent misbindings.
#[test]
fn key_to_node_matches_parser_for_golden_table() {
	use super::keymap_adapter::key_to_node;
	use xeno_keymap_core::parser::parse;
	use xeno_primitives::{Key, KeyCode, Modifiers};

	let cases: Vec<(&str, Key)> = vec![
		// Plain characters
		("a", Key::char('a')),
		("z", Key::char('z')),
		("0", Key::char('0')),
		("9", Key::char('9')),
		// Modifier + char
		("ctrl-a", Key::ctrl('a')),
		("alt-a", Key::alt('a')),
		("cmd-a", Key { code: KeyCode::Char('a'), modifiers: Modifiers::CMD }),
		("ctrl-alt-a", Key { code: KeyCode::Char('a'), modifiers: Modifiers::NONE.ctrl().alt() }),
		("cmd-alt-a", Key { code: KeyCode::Char('a'), modifiers: Modifiers::NONE.cmd().alt() }),
		// Whitespace keys (canonicalized)
		("space", Key::new(KeyCode::Space)),
		("tab", Key::new(KeyCode::Tab)),
		("enter", Key::new(KeyCode::Enter)),
		// Special keys
		("esc", Key::new(KeyCode::Esc)),
		("backspace", Key::new(KeyCode::Backspace)),
		("del", Key::new(KeyCode::Delete)),
		("insert", Key::new(KeyCode::Insert)),
		// Arrows
		("up", Key::new(KeyCode::Up)),
		("down", Key::new(KeyCode::Down)),
		("left", Key::new(KeyCode::Left)),
		("right", Key::new(KeyCode::Right)),
		// Function keys (boundary values)
		("f1", Key::new(KeyCode::F(1))),
		("f12", Key::new(KeyCode::F(12))),
		("f13", Key::new(KeyCode::F(13))),
		("f35", Key::new(KeyCode::F(35))),
		// Modifier + function keys
		("cmd-f35", Key { code: KeyCode::F(35), modifiers: Modifiers::CMD }),
		("ctrl-f13", Key { code: KeyCode::F(13), modifiers: Modifiers::CTRL }),
	];

	for (keymap_str, key) in &cases {
		let parsed = parse(keymap_str).unwrap_or_else(|e| panic!("parse({keymap_str:?}) failed: {e}"));
		let from_key = key_to_node(*key);
		assert_eq!(from_key, parsed, "mismatch for {keymap_str:?}: key_to_node={from_key:?}, parsed={parsed:?}");
	}
}

/// Golden table: `parse_seq` matches `key_to_node` for multi-key sequences.
#[test]
fn key_to_node_matches_parser_for_sequences() {
	use super::keymap_adapter::key_to_node;
	use xeno_keymap_core::parser::parse_seq;
	use xeno_primitives::Key;

	let parsed = parse_seq("g r").unwrap();
	let from_keys = vec![key_to_node(Key::char('g')), key_to_node(Key::char('r'))];
	assert_eq!(from_keys, parsed);
}
