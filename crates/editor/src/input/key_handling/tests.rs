use std::collections::HashMap;
use std::sync::Arc;

use xeno_input::input::KeyResult;
use xeno_keymap_core::parser::parse_seq;
use xeno_primitives::{Key, KeyCode};
use xeno_registry::actions::{ActionEntry, BindingMode};
use xeno_registry::config::UnresolvedKeys;
use xeno_registry::core::index::Snapshot;
use xeno_registry::{ActionId, DenseId, LookupOutcome, RegistryEntry};

use crate::Editor;
use crate::runtime::RuntimeEvent;

fn key_enter() -> Key {
	Key::new(KeyCode::Enter)
}

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
			let target_id_str = actions.interner.resolve(target_entry.id()).to_string();
			return Some((binding.mode, binding.keys.to_string(), source_id, target_id, target_id_str));
		}
	}
	None
}

fn lookup_action_id(index: &xeno_registry::KeymapSnapshot, mode: BindingMode, key_seq: &str) -> ActionId {
	let keys = parse_seq(key_seq).expect("key sequence should parse");
	match index.lookup(mode, &keys) {
		LookupOutcome::Match(entry) => entry.action_id().expect("expected action binding"),
		_ => panic!("expected a complete keybinding match"),
	}
}

#[tokio::test]
async fn enter_queues_deferred_commit_and_pump_consumes() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor.apply_runtime_event_input(RuntimeEvent::Key(key_enter())).await;
	assert!(editor.has_runtime_overlay_commit_work());

	let _ = editor.drain_until_idle(crate::runtime::DrainPolicy::for_pump()).await;
	assert!(!editor.state.ui.overlay_system.interaction().is_open());
}

#[tokio::test]
async fn effective_keymap_applies_overrides_and_invalidates_cache() {
	let mut editor = Editor::new_scratch();
	let actions = xeno_registry::ACTIONS.snapshot();
	let (mode, key_seq, base_id, target_id, target_id_str) = sample_binding(&actions).expect("registry should contain at least one binding");

	let keymap_before = editor.effective_keymap();
	assert_eq!(lookup_action_id(&keymap_before, mode, &key_seq), base_id);

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), Some(xeno_registry::Invocation::action(&target_id_str)));
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	editor.set_key_overrides(Some(UnresolvedKeys { modes }));

	let keymap_after = editor.effective_keymap();
	assert!(!Arc::ptr_eq(&keymap_before, &keymap_after));
	assert_eq!(lookup_action_id(&keymap_after, mode, &key_seq), target_id);
}

#[tokio::test]
async fn effective_keymap_continuations_include_override() {
	let mut editor = Editor::new_scratch();
	let actions = xeno_registry::ACTIONS.snapshot();
	let (_mode, _key_seq, _base_id, _target_id, target_id_str) = sample_binding(&actions).expect("registry should contain at least one binding");

	let base = editor.effective_keymap();
	let mut chosen_prefix = None;
	for action in &*actions.table {
		for binding in action.bindings.iter().filter(|b| b.mode == BindingMode::Normal) {
			let Ok(nodes) = parse_seq(&binding.keys) else {
				continue;
			};
			if nodes.len() < 2 {
				continue;
			}

			let prefix = nodes[0].to_string();
			let prefix_nodes = parse_seq(&prefix).expect("prefix should parse");
			let existing: std::collections::HashSet<String> = base
				.continuations_with_kind(BindingMode::Normal, &prefix_nodes)
				.into_iter()
				.map(|c| c.key.to_string())
				.collect();

			let candidate = ('a'..='z').map(|c| c.to_string()).find(|k| !existing.contains(k));
			if let Some(candidate) = candidate {
				chosen_prefix = Some((prefix, candidate));
				break;
			}
		}
		if chosen_prefix.is_some() {
			break;
		}
	}

	let (prefix, candidate) = chosen_prefix.expect("expected a prefix with an available continuation slot");
	let full_key = format!("{prefix} {candidate}");

	let mut normal = HashMap::new();
	normal.insert(full_key, Some(xeno_registry::Invocation::action(&target_id_str)));
	let mut modes = HashMap::new();
	modes.insert("normal".to_string(), normal);
	editor.set_key_overrides(Some(UnresolvedKeys { modes }));

	let keymap = editor.effective_keymap();
	let prefix_nodes = parse_seq(&prefix).expect("prefix should parse");
	let continuations: std::collections::HashSet<String> = keymap
		.continuations_with_kind(BindingMode::Normal, &prefix_nodes)
		.into_iter()
		.map(|c| c.key.to_string())
		.collect();

	assert!(continuations.contains(&candidate));
}

#[tokio::test]
async fn invalid_override_keeps_base_binding() {
	let mut editor = Editor::new_scratch();
	let actions = xeno_registry::ACTIONS.snapshot();
	let (mode, key_seq, base_id, _target_id, _target_id_str) = sample_binding(&actions).expect("registry should contain at least one binding");

	let mut mode_overrides = HashMap::new();
	mode_overrides.insert(key_seq.clone(), Some(xeno_registry::Invocation::action("does-not-exist")));
	let mut modes = HashMap::new();
	modes.insert(mode_name(mode).to_string(), mode_overrides);
	editor.set_key_overrides(Some(UnresolvedKeys { modes }));

	let keymap = editor.effective_keymap();
	assert_eq!(lookup_action_id(&keymap, mode, &key_seq), base_id);
}

#[test]
fn dispatch_key_result_contains_invocation() {
	let result = KeyResult::Dispatch(xeno_input::input::types::KeyDispatch {
		invocation: xeno_registry::Invocation::action("move_left"),
	});

	assert!(matches!(
		result,
		KeyResult::Dispatch(xeno_input::input::types::KeyDispatch {
			invocation: xeno_registry::Invocation::Action { name, .. }
		}) if name == "move_left"
	));
}

#[tokio::test]
async fn custom_preset_path_loads_and_sets_insert() {
	let dir = tempfile::tempdir().unwrap();
	let preset_path = dir.path().join("test_custom.nuon");
	std::fs::write(
		&preset_path,
		r#"{
    name: test_custom
    initial_mode: insert
    behavior: { vim_shift_letter_casefold: false, normal_digit_prefix_count: false }
    bindings: [
        { mode: insert, keys: "ctrl-x ctrl-s", target: "command:write" }
    ]
    prefixes: [
        { mode: insert, keys: ctrl-x, description: "Test prefix" }
    ]
}"#,
	)
	.unwrap();

	let mut editor = Editor::new_scratch();
	editor.set_keymap_preset_spec(preset_path.to_str().unwrap().to_string(), Some(dir.path()));

	assert!(
		matches!(editor.buffer().input.mode(), xeno_primitives::Mode::Insert),
		"custom preset with initial_mode: insert should start buffer in Insert mode"
	);
	assert!(!editor.keymap_behavior().vim_shift_letter_casefold);
	assert!(!editor.keymap_behavior().normal_digit_prefix_count);
}

#[tokio::test]
async fn custom_preset_ctrl_x_prefix_dispatches() {
	let dir = tempfile::tempdir().unwrap();
	let preset_path = dir.path().join("test_dispatch.nuon");
	std::fs::write(
		&preset_path,
		r#"{
    name: test_dispatch
    initial_mode: insert
    behavior: { vim_shift_letter_casefold: false, normal_digit_prefix_count: false }
    bindings: [
        { mode: insert, keys: "ctrl-x ctrl-s", target: "command:write" }
    ]
    prefixes: [
        { mode: insert, keys: ctrl-x, description: "Test prefix" }
    ]
}"#,
	)
	.unwrap();

	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	editor.set_keymap_preset_spec(preset_path.to_str().unwrap().to_string(), Some(dir.path()));

	assert!(matches!(editor.buffer().input.mode(), xeno_primitives::Mode::Insert));

	let _ = editor.handle_key(Key::ctrl('x')).await;
	assert!(
		editor.buffer().input.pending_key_count() > 0,
		"ctrl-x should produce pending state in custom preset insert mode"
	);

	let _ = editor.handle_key(Key::ctrl('s')).await;
	assert_eq!(
		editor.buffer().input.pending_key_count(),
		0,
		"pending sequence should be cleared after full chord"
	);
}

#[tokio::test]
async fn emacs_preset_starts_in_insert() {
	let mut editor = Editor::new_scratch();
	editor.set_keymap_preset("emacs".to_string());
	assert!(
		matches!(editor.buffer().input.mode(), xeno_primitives::Mode::Insert),
		"emacs preset should start buffer in Insert mode"
	);
}

#[tokio::test]
async fn emacs_ctrl_x_ctrl_s_dispatches_from_fresh_insert() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	editor.set_keymap_preset("emacs".to_string());

	// Verify we're in Insert mode
	assert!(matches!(editor.buffer().input.mode(), xeno_primitives::Mode::Insert));

	// Press ctrl-x → should be Pending
	let _ = editor.handle_key(Key::ctrl('x')).await;
	assert!(
		editor.buffer().input.pending_key_count() > 0,
		"ctrl-x should produce pending state in emacs insert mode"
	);

	// Press ctrl-s → should dispatch command:write
	// (the write may fail since there's no file, but it should dispatch)
	let _ = editor.handle_key(Key::ctrl('s')).await;
	assert_eq!(
		editor.buffer().input.pending_key_count(),
		0,
		"pending sequence should be cleared after full chord"
	);
}
