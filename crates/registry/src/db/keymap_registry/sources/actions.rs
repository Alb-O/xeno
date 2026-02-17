use std::sync::Arc;

use crate::actions::ActionEntry;
use crate::core::index::Snapshot;
use crate::core::{ActionId, DenseId, RegistryEntry, RegistrySource};
use crate::db::keymap_registry::spec::{KeymapBindingSource, KeymapSpec, SlotKey, SpecBinding, SpecBindingTarget};

pub(crate) fn collect_action_defaults(actions: &Snapshot<ActionEntry, ActionId>, spec: &mut KeymapSpec, ordinal: &mut usize) {
	let mut bindings = Vec::new();
	for (idx, action_entry) in actions.table.iter().enumerate() {
		let action_id = ActionId::from_u32(idx as u32);
		for binding in action_entry.bindings.iter() {
			bindings.push((action_id, binding.clone()));
		}
	}

	bindings.sort_by(|a, b| {
		a.1.mode
			.cmp(&b.1.mode)
			.then_with(|| a.1.keys.cmp(&b.1.keys))
			.then_with(|| a.1.priority.cmp(&b.1.priority))
			.then_with(|| a.1.action.cmp(&b.1.action))
	});

	for (action_id, binding) in bindings {
		spec.bindings.push(SpecBinding {
			source: KeymapBindingSource::ActionDefault,
			ordinal: *ordinal,
			slot: SlotKey {
				mode: binding.mode,
				sequence: Arc::clone(&binding.keys),
			},
			target: SpecBindingTarget::Action {
				id: action_id,
				count: 1,
				extend: false,
				register: None,
			},
			target_desc: Arc::from(canonical_action_id(actions, action_id).as_str()),
			priority: binding.priority,
		});
		*ordinal += 1;
	}
}

pub(crate) fn collect_runtime_action_bindings(actions: &Snapshot<ActionEntry, ActionId>, spec: &mut KeymapSpec, ordinal: &mut usize) {
	for (idx, action_entry) in actions.table.iter().enumerate() {
		if !matches!(action_entry.source(), RegistrySource::Runtime) {
			continue;
		}

		let action_id = ActionId::from_u32(idx as u32);
		for binding in action_entry.bindings.iter() {
			spec.bindings.push(SpecBinding {
				source: KeymapBindingSource::RuntimeAction,
				ordinal: *ordinal,
				slot: SlotKey {
					mode: binding.mode,
					sequence: Arc::clone(&binding.keys),
				},
				target: SpecBindingTarget::Action {
					id: action_id,
					count: 1,
					extend: false,
					register: None,
				},
				target_desc: Arc::from(canonical_action_id(actions, action_id).as_str()),
				priority: binding.priority,
			});
			*ordinal += 1;
		}
	}
}

fn canonical_action_id(actions: &Snapshot<ActionEntry, ActionId>, action_id: ActionId) -> String {
	let action_entry = &actions.table[action_id.as_u32() as usize];
	actions.interner.resolve(action_entry.id()).to_string()
}
