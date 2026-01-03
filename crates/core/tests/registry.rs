//! Integration tests for the core registry system.
//!
//! These tests verify that the registry infrastructure is correctly wired up,
//! including action ID resolution, result handlers, and keymap lookups.

use xeno_core::{
	LookupResult, find_action, find_action_by_id, get_keymap_registry, resolve_action_id,
};
use xeno_keymap::parser::parse_seq;
use xeno_registry::BindingMode;
use xeno_registry::actions::{
	RESULT_CURSOR_MOVE_HANDLERS, RESULT_EDIT_HANDLERS, RESULT_ERROR_HANDLERS,
	RESULT_INSERT_WITH_MOTION_HANDLERS, RESULT_MODE_CHANGE_HANDLERS, RESULT_MOTION_HANDLERS,
	RESULT_OK_HANDLERS, RESULT_QUIT_HANDLERS,
};

#[test]
fn test_action_id_resolution() {
	let move_left_id = resolve_action_id("move_left");
	assert!(
		move_left_id.is_some(),
		"move_left should resolve to ActionId"
	);
	let id = move_left_id.unwrap();
	assert!(id.is_valid(), "ActionId should be valid");

	let action = find_action_by_id(id);
	assert!(action.is_some(), "should find action by ActionId");
	assert_eq!(action.unwrap().name, "move_left");

	let invalid = find_action_by_id(xeno_core::ActionId::INVALID);
	assert!(invalid.is_none(), "INVALID ActionId should return None");

	let by_name = find_action("move_left").unwrap();
	let by_id = find_action_by_id(id).unwrap();
	assert_eq!(
		by_name.name, by_id.name,
		"find_action and find_action_by_id should return the same action"
	);
}

#[test]
fn test_handlers_registered() {
	// Ensure common variants have at least one handler registered.
	assert!(!RESULT_OK_HANDLERS.is_empty());
	assert!(!RESULT_QUIT_HANDLERS.is_empty());
	assert!(!RESULT_ERROR_HANDLERS.is_empty());
}

#[test]
fn test_handler_coverage_counts() {
	let total = RESULT_OK_HANDLERS.len()
		+ RESULT_MODE_CHANGE_HANDLERS.len()
		+ RESULT_CURSOR_MOVE_HANDLERS.len()
		+ RESULT_MOTION_HANDLERS.len()
		+ RESULT_INSERT_WITH_MOTION_HANDLERS.len()
		+ RESULT_EDIT_HANDLERS.len()
		+ RESULT_QUIT_HANDLERS.len()
		+ RESULT_ERROR_HANDLERS.len();
	assert!(
		total >= 8,
		"expected handlers registered for major variants"
	);
}

#[test]
fn test_keymap_registry_lookup() {
	let registry = get_keymap_registry();

	// Test that "h" is bound to move_left in normal mode
	let keys = parse_seq("h").unwrap();
	match registry.lookup(BindingMode::Normal, &keys) {
		LookupResult::Match(entry) => {
			assert_eq!(entry.action_name, "move_left");
			assert!(entry.action_id.is_valid(), "ActionId should be valid");

			// Verify round-trip: ActionId should map back to same action
			let action = find_action_by_id(entry.action_id);
			assert!(action.is_some());
			assert_eq!(action.unwrap().name, "move_left");
		}
		other => panic!("Expected Match for 'h', got {other:?}"),
	}
}
