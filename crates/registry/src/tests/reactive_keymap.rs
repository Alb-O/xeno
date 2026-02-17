use std::sync::Arc;

use crate::actions::{ActionContext, ActionDef, ActionEffects, ActionResult, BindingMode, KeyBindingDef};
use crate::core::{RegistryMetaStatic, RegistrySource};
use crate::db::ACTIONS;
use crate::db::keymap_registry::get_keymap_snapshot;

#[test]
fn test_reactive_keymap_updates() {
	// 0. Pin old snapshot
	let old_snap = ACTIONS.snapshot();

	// 1. Initial state: check if a binding exists
	let keymap = get_keymap_snapshot();
	let test_keys = xeno_keymap_core::parser::parse_seq("ctrl-alt-shift-t").unwrap();

	match keymap.lookup(BindingMode::Normal, &test_keys) {
		crate::db::keymap_registry::LookupOutcome::None => {}
		_ => panic!("Test key sequence already bound!"),
	}

	// 2. Register a new action with that binding
	// We use Box::leak to get 'static for ActionDef in a test environment
	let def: &'static ActionDef = Box::leak(Box::new(ActionDef {
		meta: RegistryMetaStatic {
			id: "test::reactive_action",
			name: "reactive_action",
			keys: &[],
			description: "Test action for reactive keymap",
			priority: 100,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
		short_desc: "test",
		handler: test_handler,
		bindings: Box::leak(Box::new([KeyBindingDef {
			mode: BindingMode::Normal,
			keys: Arc::from("ctrl-alt-shift-t"),
			action: Arc::from("test::reactive_action"),
			priority: 100,
		}])),
	}));

	fn test_handler(_ctx: &ActionContext) -> ActionResult {
		ActionResult::Effects(ActionEffects::default())
	}

	ACTIONS.register(def).expect("Failed to register test action");

	// 3. Verify that the old snapshot still doesn't have the binding (Isolation)
	let old_keymap = crate::db::get_db().keymap.for_snapshot(old_snap);
	match old_keymap.lookup(BindingMode::Normal, &test_keys) {
		crate::db::keymap_registry::LookupOutcome::None => {}
		_ => panic!("Old keymap incorrectly sees the new binding! Isolation failed."),
	}

	// 4. Verify that the new keymap includes the binding
	let new_keymap = get_keymap_snapshot();

	match new_keymap.lookup(BindingMode::Normal, &test_keys) {
		crate::db::keymap_registry::LookupOutcome::Match(entry) => {
			assert_eq!(entry.name(), "reactive_action");
		}
		_ => panic!("New keymap does not contain the reactive binding!"),
	}
}
