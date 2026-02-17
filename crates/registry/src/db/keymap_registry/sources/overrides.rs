use std::sync::Arc;

use super::super::spec::{KeymapBindingSource, KeymapSpec, SlotKey, SpecBinding, SpecBindingTarget, parse_binding_mode};
use crate::actions::BindingMode;
use crate::config::UnresolvedKeys;
use crate::invocation::Invocation;

pub(crate) fn collect_overrides(overrides: &UnresolvedKeys, spec: &mut KeymapSpec, ordinal: &mut usize) {
	let mut entries: Vec<(BindingMode, Arc<str>, Option<Invocation>)> = Vec::new();
	for (mode_name, key_map) in &overrides.modes {
		let Some(mode) = parse_binding_mode(mode_name) else {
			continue;
		};
		for (key_seq, opt_inv) in key_map {
			entries.push((mode, Arc::from(key_seq.as_str()), opt_inv.clone()));
		}
	}

	entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

	for (mode, sequence, opt_inv) in entries {
		let (target, target_desc) = match opt_inv {
			Some(inv) => {
				let desc = Arc::from(inv.describe().as_str());
				(SpecBindingTarget::Invocation(inv), desc)
			}
			None => (SpecBindingTarget::Unbind, Arc::from("<unbind>")),
		};

		spec.bindings.push(SpecBinding {
			source: KeymapBindingSource::Override,
			ordinal: *ordinal,
			slot: SlotKey { mode, sequence },
			target,
			target_desc,
			priority: i16::MIN,
		});
		*ordinal += 1;
	}
}
