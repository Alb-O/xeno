//! Integration tests for the core registry system.
//!
//! These tests verify that the registry infrastructure is correctly wired up,
//! including action ID resolution, result handlers, and keymap lookups.

use xeno_core::{
	LookupResult, find_action, find_action_by_id, get_keymap_registry, resolve_action_id,
};
use xeno_keymap_core::parser::parse_seq;
use xeno_registry::BindingMode;
use xeno_registry::actions::RESULT_EFFECTS_HANDLERS;

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
fn test_effects_handler_registered() {
	assert!(!RESULT_EFFECTS_HANDLERS.is_empty());
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
