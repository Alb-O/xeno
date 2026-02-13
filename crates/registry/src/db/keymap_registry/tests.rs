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
		LookupResult::Match(entry) => entry.action_id,
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
