mod actions;
mod overrides;
mod preset;

use super::spec::KeymapSpec;
use crate::actions::ActionEntry;
use crate::config::UnresolvedKeys;
use crate::core::ActionId;
use crate::core::index::Snapshot;
use crate::keymaps::KeymapPreset;

/// Collect source bindings and prefix metadata from preset/default/override inputs.
pub(crate) fn collect_keymap_spec(actions: &Snapshot<ActionEntry, ActionId>, preset: Option<&KeymapPreset>, overrides: Option<&UnresolvedKeys>) -> KeymapSpec {
	let mut spec = KeymapSpec::default();
	let mut ordinal = 0usize;

	if let Some(preset) = preset {
		preset::collect_preset_bindings(actions, preset, &mut spec, &mut ordinal);
		actions::collect_runtime_action_bindings(actions, &mut spec, &mut ordinal);
		preset::collect_preset_prefixes(preset, &mut spec);
	} else {
		actions::collect_action_defaults(actions, &mut spec, &mut ordinal);
	}

	if let Some(overrides) = overrides {
		overrides::collect_overrides(overrides, &mut spec, &mut ordinal);
	}

	spec
}
