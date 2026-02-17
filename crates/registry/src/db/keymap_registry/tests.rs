use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use xeno_keymap_core::parser::parse_seq;

use super::*;
use crate::actions::{ActionEntry, BindingMode};
use crate::config::UnresolvedKeys;
use crate::core::index::Snapshot;
use crate::core::{ActionId, DenseId, RegistryEntry};
use crate::invocation::Invocation;

static RUNTIME_OVERRIDE_SEQ: AtomicU64 = AtomicU64::new(0);

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

fn lookup_action_id(index: &KeymapSnapshot, mode: BindingMode, key_seq: &str) -> ActionId {
	let keys = parse_seq(key_seq).expect("binding key sequence should parse");
	match index.lookup(mode, &keys) {
		LookupOutcome::Match(entry) => entry.action_id().expect("expected action binding"),
		_ => panic!("expected a complete keybinding match"),
	}
}

fn leak_str(value: String) -> &'static str {
	Box::leak(value.into_boxed_str())
}

fn register_runtime_action_binding(key: &str, priority: i16) -> String {
	use crate::actions::{ActionContext, ActionDef, ActionEffects, ActionResult, KeyBindingDef};
	use crate::core::{RegistryMetaStatic, RegistrySource};

	fn handler(_ctx: &ActionContext) -> ActionResult {
		ActionResult::Effects(ActionEffects::default())
	}

	let seq = RUNTIME_OVERRIDE_SEQ.fetch_add(1, Ordering::Relaxed);
	let canonical_id = format!("test::runtime_override_binding_{seq}");
	let name = format!("runtime_override_binding_{seq}");
	let short_desc = format!("rt_override_{seq}");

	let def: &'static ActionDef = Box::leak(Box::new(ActionDef {
		meta: RegistryMetaStatic {
			id: leak_str(canonical_id.clone()),
			name: leak_str(name),
			keys: &[],
			description: "runtime keymap override test action",
			priority,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
		short_desc: leak_str(short_desc),
		handler,
		bindings: Box::leak(Box::new([KeyBindingDef {
			mode: BindingMode::Normal,
			keys: Arc::from(key),
			action: Arc::from(canonical_id.as_str()),
			priority,
		}])),
	}));

	crate::db::ACTIONS.register(def).expect("runtime action registration should succeed");
	canonical_id
}

#[test]
fn override_wins_over_base_binding() {
	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, _base_id, target_id, target_name) = sample_binding(&actions).expect("registry should contain at least one binding");

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), Some(Invocation::action(&target_name)));
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapSnapshot::build_with_overrides(&actions, Some(&overrides));
	let resolved = lookup_action_id(&index, mode, &key_seq);
	assert_eq!(resolved, target_id);
}

#[test]
fn invalid_override_action_keeps_base_binding() {
	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, base_id, _target_id, _target_name) = sample_binding(&actions).expect("registry should contain at least one binding");

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), Some(Invocation::action("does-not-exist")));
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapSnapshot::build_with_overrides(&actions, Some(&overrides));
	let resolved = lookup_action_id(&index, mode, &key_seq);
	assert_eq!(resolved, base_id);
}

#[test]
fn invocation_override_in_trie() {
	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, base_id, _target_id, _target_name) = sample_binding(&actions).expect("registry should contain at least one binding");
	let base_action_id_str = {
		let entry = &actions.table[base_id.as_u32() as usize];
		actions.interner.resolve(entry.id()).to_string()
	};

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), Some(Invocation::editor_command("stats", vec![])));
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapSnapshot::build_with_overrides(&actions, Some(&overrides));
	let keys = parse_seq(&key_seq).expect("key sequence should parse");
	match index.lookup(mode, &keys) {
		LookupOutcome::Match(entry) => {
			assert!(matches!(
				entry.target(),
				CompiledBindingTarget::Invocation {
					inv: Invocation::Command(xeno_invocation::CommandInvocation {
						name,
						route: xeno_invocation::CommandRoute::Editor,
						..
					})
				} if name == "stats"
			));
		}
		_ => panic!("expected a complete keybinding match for invocation override"),
	}
	assert!(!index.conflicts().is_empty(), "overriding a base binding should record a conflict");
	let conflict = index
		.conflicts()
		.iter()
		.find(|c| c.keys.as_ref() == key_seq)
		.expect("conflict for overridden key");
	assert_eq!(
		conflict.dropped_target, base_action_id_str,
		"dropped_target should be the original base binding"
	);
}

#[test]
fn invocation_override_fresh_key() {
	let actions = crate::db::ACTIONS.snapshot();

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert("ctrl-f12".to_string(), Some(Invocation::command("write", vec![])));
	let mut modes = HashMap::new();
	modes.insert("normal".to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapSnapshot::build_with_overrides(&actions, Some(&overrides));
	let keys = parse_seq("ctrl-f12").expect("key sequence should parse");
	match index.lookup(BindingMode::Normal, &keys) {
		LookupOutcome::Match(entry) => {
			assert!(matches!(
				entry.target(),
				CompiledBindingTarget::Invocation {
					inv: Invocation::Command(xeno_invocation::CommandInvocation { name, .. })
				} if name == "write"
			));
			assert_eq!(entry.short_desc(), "write");
		}
		_ => panic!("expected invocation match for fresh key"),
	}
}

#[test]
fn invocation_override_nu_target() {
	let actions = crate::db::ACTIONS.snapshot();

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert("ctrl-f11".to_string(), Some(Invocation::nu("go", vec!["fast".to_string()])));
	let mut modes = HashMap::new();
	modes.insert("normal".to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapSnapshot::build_with_overrides(&actions, Some(&overrides));
	let keys = parse_seq("ctrl-f11").expect("key sequence should parse");
	match index.lookup(BindingMode::Normal, &keys) {
		LookupOutcome::Match(entry) => {
			assert!(matches!(
				entry.target(),
				CompiledBindingTarget::Invocation { inv: Invocation::Nu { name, args } } if name == "go" && args == &["fast".to_string()]
			));
			assert_eq!(entry.short_desc(), "go");
		}
		_ => panic!("expected nu invocation match for fresh key"),
	}
}

#[test]
fn which_key_labels_invocation() {
	let actions = crate::db::ACTIONS.snapshot();

	let mut normal = HashMap::new();
	normal.insert("g r".to_string(), Some(Invocation::editor_command("reload_config", vec![])));
	let mut modes = HashMap::new();
	modes.insert("normal".to_string(), normal);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapSnapshot::build_with_overrides(&actions, Some(&overrides));
	let g_key = parse_seq("g").expect("should parse");
	let continuations = index.continuations_with_kind(BindingMode::Normal, &g_key);

	let r_cont = continuations
		.iter()
		.find(|c| c.key.to_string() == "r")
		.expect("should have 'r' continuation under 'g'");
	let entry = r_cont.value.expect("leaf should have a binding entry");
	assert_eq!(entry.short_desc(), "reload_config");
	assert!(matches!(
		entry.target(),
		CompiledBindingTarget::Invocation {
			inv: Invocation::Command(xeno_invocation::CommandInvocation {
				route: xeno_invocation::CommandRoute::Editor,
				..
			})
		}
	));
}

#[test]
fn invalid_override_produces_problem() {
	use super::KeymapProblemKind;

	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, base_id, _, _) = sample_binding(&actions).expect("registry should contain at least one binding");

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), Some(Invocation::action("does-not-exist")));
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapSnapshot::build_with_overrides(&actions, Some(&overrides));

	// Base binding should remain for the bad action target
	let resolved = lookup_action_id(&index, mode, &key_seq);
	assert_eq!(resolved, base_id);

	// Should have problems
	assert!(!index.problems().is_empty(), "should have build problems for invalid overrides");
	let unknown_action = index.problems().iter().find(|p| p.kind == KeymapProblemKind::UnknownActionTarget);
	assert!(unknown_action.is_some(), "should have UnknownActionTarget problem");
}

#[test]
fn unbind_removes_base_binding() {
	let actions = crate::db::ACTIONS.snapshot();
	let (mode, key_seq, _base_id, _, _) = sample_binding(&actions).expect("registry should contain at least one binding");

	// Verify binding exists in base
	let base_index = KeymapSnapshot::build(&actions);
	let keys = parse_seq(&key_seq).expect("key sequence should parse");
	assert!(
		matches!(base_index.lookup(mode, &keys), LookupOutcome::Match(_)),
		"base binding should exist before unbind"
	);

	// Unbind it
	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), None);
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	let overrides = UnresolvedKeys { modes };

	let index = KeymapSnapshot::build_with_overrides(&actions, Some(&overrides));
	assert!(matches!(index.lookup(mode, &keys), LookupOutcome::None), "unbound key should produce no match");
}

#[test]
fn preset_emacs_loads() {
	let preset = crate::keymaps::preset("emacs").expect("emacs preset must load");

	assert_eq!(&*preset.name, "emacs");
	assert!(matches!(preset.initial_mode, xeno_primitives::Mode::Insert));
	assert!(!preset.behavior.vim_shift_letter_casefold);
	assert!(!preset.behavior.normal_digit_prefix_count);
	assert!(!preset.bindings.is_empty(), "emacs preset should have bindings");
	assert!(!preset.prefixes.is_empty(), "emacs preset should have prefixes");

	// C-x prefix should be present
	let has_cx_prefix = preset.prefixes.iter().any(|p| &*p.keys == "ctrl-x");
	assert!(has_cx_prefix, "emacs preset should have ctrl-x prefix");

	// Build should succeed
	let actions = crate::db::ACTIONS.snapshot();
	let index = KeymapSnapshot::build_with_preset(&actions, Some(&preset), None);

	// ctrl-x ctrl-s should resolve
	let keys = parse_seq("ctrl-x ctrl-s").expect("ctrl-x ctrl-s should parse");
	assert!(
		matches!(index.lookup(BindingMode::Insert, &keys), LookupOutcome::Match(_)),
		"emacs C-x C-s should resolve to a binding"
	);

	// ctrl-x alone should be Pending
	let prefix = parse_seq("ctrl-x").expect("ctrl-x should parse");
	assert!(
		matches!(index.lookup(BindingMode::Insert, &prefix), LookupOutcome::Pending { .. }),
		"emacs ctrl-x should be Pending"
	);
}

#[test]
fn preset_binding_precedes_runtime_action_binding() {
	let key = "ctrl-alt-shift-r";
	let runtime_id = register_runtime_action_binding(key, 10);
	let actions = crate::db::ACTIONS.snapshot();

	let preset_target_id = actions
		.table
		.iter()
		.find_map(|entry| {
			let id = actions.interner.resolve(entry.id());
			(id != runtime_id).then(|| id.to_string())
		})
		.expect("expected at least one non-runtime action id");

	let preset = crate::keymaps::KeymapPreset {
		name: Arc::from("runtime_override_test"),
		initial_mode: xeno_primitives::Mode::Normal,
		behavior: crate::keymaps::KeymapBehavior::default(),
		bindings: vec![crate::keymaps::PresetBinding {
			mode: "normal".to_string(),
			keys: Arc::from(key),
			target: format!("action:{preset_target_id}"),
		}],
		prefixes: Vec::new(),
	};

	let index = KeymapSnapshot::build_with_preset(&actions, Some(&preset), None);
	let resolved_id = lookup_action_id(&index, BindingMode::Normal, key);
	let resolved_entry = &actions.table[resolved_id.as_u32() as usize];
	let resolved_target = actions.interner.resolve(resolved_entry.id());
	assert_eq!(resolved_target, preset_target_id);
}
