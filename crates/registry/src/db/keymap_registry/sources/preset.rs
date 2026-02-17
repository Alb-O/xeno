use std::sync::Arc;

use super::super::diagnostics::{KeymapProblemKind, push_problem};
use super::super::spec::{KeymapBindingSource, KeymapSpec, SlotKey, SpecBinding, SpecBindingTarget, SpecPrefix, parse_binding_mode};
use crate::actions::ActionEntry;
use crate::core::ActionId;
use crate::core::index::Snapshot;
use crate::invocation::Invocation;
use crate::keymaps::KeymapPreset;

pub(crate) fn collect_preset_bindings(actions: &Snapshot<ActionEntry, ActionId>, preset: &KeymapPreset, spec: &mut KeymapSpec, ordinal: &mut usize) {
	for binding in &preset.bindings {
		let Some(mode) = parse_binding_mode(&binding.mode) else {
			continue;
		};

		let target_desc: Arc<str> = Arc::from(binding.target.as_str());
		let inv = match xeno_invocation_spec::parse_spec(&binding.target) {
			Ok(parsed) => match parsed.kind {
				xeno_invocation_spec::SpecKind::Action => Invocation::action(&parsed.name),
				xeno_invocation_spec::SpecKind::Command => Invocation::command(&parsed.name, parsed.args),
				xeno_invocation_spec::SpecKind::Editor => Invocation::editor_command(&parsed.name, parsed.args),
				xeno_invocation_spec::SpecKind::Nu => Invocation::nu(&parsed.name, parsed.args),
			},
			Err(_) => {
				push_problem(
					&mut spec.problems,
					Some(mode),
					&binding.keys,
					&target_desc,
					KeymapProblemKind::InvalidTargetSpec,
					"invalid target spec in preset",
				);
				continue;
			}
		};

		spec.bindings.push(SpecBinding {
			source: KeymapBindingSource::Preset,
			ordinal: *ordinal,
			slot: SlotKey {
				mode,
				sequence: Arc::clone(&binding.keys),
			},
			target: SpecBindingTarget::Invocation(inv),
			target_desc,
			priority: 100,
		});
		*ordinal += 1;
	}

	let _ = actions;
}

pub(crate) fn collect_preset_prefixes(preset: &KeymapPreset, spec: &mut KeymapSpec) {
	spec.prefixes.extend(preset.prefixes.iter().filter_map(|prefix| {
		let mode = parse_binding_mode(&prefix.mode)?;
		Some(SpecPrefix {
			mode,
			keys: Arc::clone(&prefix.keys),
			description: Arc::clone(&prefix.description),
		})
	}));
}
