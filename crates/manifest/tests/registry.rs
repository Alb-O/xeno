//! Integration tests for the manifest registry system.
//!
//! These tests verify that the registry is correctly populated when
//! evildoer-stdlib is linked. They test the integration between manifest
//! (which defines slices) and stdlib (which populates them).

// Force linkage of evildoer-stdlib to ensure all registrations occur.
extern crate evildoer_stdlib;

use evildoer_base::key::Key;
use evildoer_manifest::actions::{
	RESULT_CURSOR_MOVE_HANDLERS, RESULT_EDIT_HANDLERS, RESULT_ERROR_HANDLERS,
	RESULT_INSERT_WITH_MOTION_HANDLERS, RESULT_MODE_CHANGE_HANDLERS, RESULT_MOTION_HANDLERS,
	RESULT_OK_HANDLERS, RESULT_QUIT_HANDLERS,
};
use evildoer_manifest::{
	BindingMode, LANGUAGES, find_action, find_action_by_id, find_binding_resolved,
	resolve_action_id,
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

	let invalid = find_action_by_id(evildoer_manifest::ActionId::INVALID);
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
fn test_find_binding_resolved() {
	// Test that find_binding_resolved returns ActionId
	let resolved = find_binding_resolved(BindingMode::Normal, Key::char('h'));
	assert!(resolved.is_some(), "should find binding for 'h'");
	let resolved = resolved.unwrap();
	assert_eq!(resolved.binding.action, "move_left");
	assert!(resolved.action_id.is_valid(), "ActionId should be valid");

	// Verify round-trip: resolved ActionId should map back to same action
	let action = find_action_by_id(resolved.action_id);
	assert!(action.is_some());
	assert_eq!(action.unwrap().name, "move_left");
}

#[test]
fn test_languages_registered() {
	// With evildoer_stdlib linked, we should have languages registered
	assert!(
		!LANGUAGES.is_empty(),
		"LANGUAGES should contain entries from evildoer-stdlib"
	);

	// Check for common languages
	let has_rust = LANGUAGES.iter().any(|l| l.name == "rust");
	assert!(has_rust, "should have rust language registered");

	let has_python = LANGUAGES.iter().any(|l| l.name == "python");
	assert!(has_python, "should have python language registered");
}
