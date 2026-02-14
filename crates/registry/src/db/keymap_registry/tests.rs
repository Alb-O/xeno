use std::collections::HashMap;

use super::*;

fn mode_name(mode: BindingMode) -> &'static str {
	match mode {
		BindingMode::Normal => "normal",
		BindingMode::Insert => "insert",
		BindingMode::Match => "match",
		BindingMode::Space => "space",
	}
}

fn sample_binding(actions: &Snapshot<ActionEntry, ActionId>) -> Option<(BindingMode, String, ActionId, ActionId, String)> {
	for (idx, action_entry) in actions.table.iter().enumerate() {
		let source_id = ActionId::from_u32(idx as u32);
		for binding in action_entry.bindings.iter() {
			if parse_seq(&binding.keys).is_err() {
				continue;
			}

			let Some((target_idx, target_entry)) = actions.table.iter().enumerate().find(|(target_idx, _)| *target_idx != idx) else {
				continue;
			};

			let target_id = ActionId::from_u32(target_idx as u32);
			let target_name = actions.interner.resolve(target_entry.name()).to_string();
			return Some((binding.mode, binding.keys.to_string(), source_id, target_id, target_name));
		}
	}
	None
}

fn lookup_action_id(index: &KeymapIndex, mode: BindingMode, key_seq: &str) -> ActionId {
	let keys = parse_seq(key_seq).expect("binding key sequence should parse");
	match index.lookup(mode, &keys) {
		LookupResult::Match(entry) => entry.action_id().expect("expected action binding"),
		_ => panic!("expected a complete keybinding match"),
	}
}

#[test]
fn override_wins_over_base_binding() {
	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, _base_id, target_id, target_name) = sample_binding(&actions).expect("registry should contain at least one binding");

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), target_name);
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapIndex::build_with_overrides(&actions, Some(&overrides));
	let resolved = lookup_action_id(&index, mode, &key_seq);
	assert_eq!(resolved, target_id);
}

#[test]
fn invalid_override_action_keeps_base_binding() {
	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, base_id, _target_id, _target_name) = sample_binding(&actions).expect("registry should contain at least one binding");

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), "action:does-not-exist".to_string());
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapIndex::build_with_overrides(&actions, Some(&overrides));
	let resolved = lookup_action_id(&index, mode, &key_seq);
	assert_eq!(resolved, base_id);
}

#[test]
fn invocation_spec_override_in_trie() {
	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, base_id, _target_id, _target_name) = sample_binding(&actions).expect("registry should contain at least one binding");
	let base_action_id_str = {
		let entry = &actions.table[base_id.as_u32() as usize];
		actions.interner.resolve(entry.id()).to_string()
	};

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), "editor:stats".to_string());
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapIndex::build_with_overrides(&actions, Some(&overrides));
	let keys = parse_seq(&key_seq).expect("key sequence should parse");
	match index.lookup(mode, &keys) {
		LookupResult::Match(entry) => {
			assert!(matches!(&entry.target, BindingTarget::InvocationSpec { spec, .. } if spec.as_ref() == "editor:stats"));
		}
		_ => panic!("expected a complete keybinding match for invocation spec override"),
	}
	assert!(!index.conflicts().is_empty(), "overriding a base binding should record a conflict");
	let conflict = index
		.conflicts()
		.iter()
		.find(|c| c.keys.as_ref() == key_seq)
		.expect("conflict for overridden key");
	assert_eq!(conflict.kept_target, "editor:stats", "kept_target should be the override winner");
	assert_eq!(
		conflict.dropped_target, base_action_id_str,
		"dropped_target should be the original base binding"
	);
}

#[test]
fn invocation_spec_override_fresh_key() {
	let actions = crate::db::ACTIONS.snapshot();

	let mut mode_overrides = HashMap::new();
	// Use a key sequence unlikely to exist in base bindings
	mode_overrides.insert("ctrl-f12".to_string(), "command:write".to_string());
	let mut modes = HashMap::new();
	modes.insert("normal".to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapIndex::build_with_overrides(&actions, Some(&overrides));
	let keys = parse_seq("ctrl-f12").expect("key sequence should parse");
	match index.lookup(BindingMode::Normal, &keys) {
		LookupResult::Match(entry) => {
			assert!(matches!(&entry.target, BindingTarget::InvocationSpec { spec, .. } if spec.as_ref() == "command:write"));
			assert_eq!(&*entry.short_desc, "write");
		}
		_ => panic!("expected invocation spec match for fresh key"),
	}
}

#[test]
fn which_key_labels_invocation_spec() {
	let actions = crate::db::ACTIONS.snapshot();

	let mut normal = HashMap::new();
	normal.insert("g r".to_string(), "editor:reload_config".to_string());
	let mut modes = HashMap::new();
	modes.insert("normal".to_string(), normal);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapIndex::build_with_overrides(&actions, Some(&overrides));
	let g_key = parse_seq("g").expect("should parse");
	let continuations = index.continuations_with_kind(BindingMode::Normal, &g_key);

	let r_cont = continuations
		.iter()
		.find(|c| c.key.to_string() == "r")
		.expect("should have 'r' continuation under 'g'");
	let entry = r_cont.value.expect("leaf should have a binding entry");
	assert_eq!(&*entry.short_desc, "reload_config");
	assert!(matches!(
		&entry.target,
		BindingTarget::InvocationSpec {
			kind: xeno_invocation_spec::SpecKind::Editor,
			..
		}
	));
}

#[test]
fn invalid_override_produces_problem() {
	use super::KeymapProblemKind;

	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, base_id, _, _) = sample_binding(&actions).expect("registry should contain at least one binding");

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), "action:does-not-exist".to_string());
	// Also add an invalid key sequence
	mode_overrides.insert("g [".to_string(), "editor:stats".to_string());
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapIndex::build_with_overrides(&actions, Some(&overrides));

	// Base binding should remain for the bad action target
	let resolved = lookup_action_id(&index, mode, &key_seq);
	assert_eq!(resolved, base_id);

	// Should have problems
	assert!(!index.problems().is_empty(), "should have build problems for invalid overrides");
	let unknown_action = index.problems().iter().find(|p| p.kind == KeymapProblemKind::UnknownActionTarget);
	assert!(unknown_action.is_some(), "should have UnknownActionTarget problem");
}
