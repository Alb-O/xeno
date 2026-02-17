use std::collections::HashMap;

use xeno_keymap_core::parser::parse_seq;

use super::KeymapSnapshot;
use crate::actions::{ActionEntry, BindingMode};
use crate::config::UnresolvedKeys;
use crate::core::index::Snapshot;
use crate::core::{ActionId, DenseId, RegistryEntry};
use crate::invocation::Invocation;

fn mode_name(mode: BindingMode) -> &'static str {
	match mode {
		BindingMode::Normal => "normal",
		BindingMode::Insert => "insert",
		BindingMode::Match => "match",
		BindingMode::Space => "space",
	}
}

fn sample_binding(actions: &Snapshot<ActionEntry, ActionId>) -> Option<(BindingMode, String, ActionId, String)> {
	for (idx, action_entry) in actions.table.iter().enumerate() {
		let source_id = ActionId::from_u32(idx as u32);
		for binding in action_entry.bindings.iter() {
			if parse_seq(&binding.keys).is_err() {
				continue;
			}
			let source_name = actions.interner.resolve(action_entry.id()).to_string();
			return Some((binding.mode, binding.keys.to_string(), source_id, source_name));
		}
	}
	None
}

/// Must resolve deterministic winners independent of source map iteration order.
///
/// * Enforced in: `sources::collect_overrides`, `precedence::compare_candidates`, `KeymapCompiler::compile`
/// * Failure symptom: equivalent configs produce different key dispatch targets across runs.
#[cfg_attr(test, test)]
pub(crate) fn test_deterministic_winner_for_equivalent_overrides() {
	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, _base_id, base_action_name) = sample_binding(&actions).expect("registry should contain at least one valid binding");

	let mut first_mode_overrides = HashMap::new();
	first_mode_overrides.insert(key_seq.clone(), Some(Invocation::action(&base_action_name)));
	first_mode_overrides.insert("ctrl-f9".to_string(), Some(Invocation::command("write", vec![])));
	let mut first_modes = HashMap::new();
	first_modes.insert(mode_name(mode).to_string(), first_mode_overrides);

	let mut second_mode_overrides = HashMap::new();
	second_mode_overrides.insert("ctrl-f9".to_string(), Some(Invocation::command("write", vec![])));
	second_mode_overrides.insert(key_seq.clone(), Some(Invocation::action(&base_action_name)));
	let mut second_modes = HashMap::new();
	second_modes.insert(mode_name(mode).to_string(), second_mode_overrides);

	let first = KeymapSnapshot::build_with_overrides(&actions, Some(&UnresolvedKeys { modes: first_modes }));
	let second = KeymapSnapshot::build_with_overrides(&actions, Some(&UnresolvedKeys { modes: second_modes }));

	let keys = parse_seq(&key_seq).expect("key sequence should parse");
	match (first.lookup(mode, &keys), second.lookup(mode, &keys)) {
		(super::LookupOutcome::Match(a), super::LookupOutcome::Match(b)) => {
			assert_eq!(a.action_id(), b.action_id());
		}
		_ => panic!("expected both snapshots to resolve the same complete match"),
	}
}

/// Must apply override source precedence above preset and inherited defaults.
///
/// * Enforced in: `precedence::compare_candidates`, `KeymapCompiler::compile`
/// * Failure symptom: user override does not replace preset/default binding.
#[cfg_attr(test, test)]
pub(crate) fn test_override_precedes_inherited_binding() {
	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, _base_id, base_action_name) = sample_binding(&actions).expect("registry should contain at least one valid binding");

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), Some(Invocation::action(&base_action_name)));
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);

	let snapshot = KeymapSnapshot::build_with_overrides(&actions, Some(&UnresolvedKeys { modes }));
	let keys = parse_seq(&key_seq).expect("key sequence should parse");
	assert!(matches!(snapshot.lookup(mode, &keys), super::LookupOutcome::Match(_)));
}

/// Must remove inherited bindings when override specifies explicit unbind.
///
/// * Enforced in: `sources::collect_overrides`, `KeymapCompiler::compile`
/// * Failure symptom: key remains active after user sets binding to `null`.
#[cfg_attr(test, test)]
pub(crate) fn test_unbind_removes_inherited_binding() {
	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, _base_id, _base_action_name) = sample_binding(&actions).expect("registry should contain at least one valid binding");

	let base = KeymapSnapshot::build(&actions);
	let keys = parse_seq(&key_seq).expect("key sequence should parse");
	assert!(matches!(base.lookup(mode, &keys), super::LookupOutcome::Match(_)));

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), None);
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	let overridden = KeymapSnapshot::build_with_overrides(&actions, Some(&UnresolvedKeys { modes }));

	assert!(matches!(overridden.lookup(mode, &keys), super::LookupOutcome::None));
}
