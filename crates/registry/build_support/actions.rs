//! NUON → [`ActionsSpec`] compiler.

use crate::build_support::compile::*;
use crate::schema::actions::{ActionsSpec, VALID_CAPS, VALID_MODES};

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/actions/assets/actions.nuon");
	ctx.rerun_if_changed(&path);

	let spec: ActionsSpec = read_nuon_spec(&path);

	validate_unique(spec.actions.iter().map(|action| action.common.name.as_str()), "action");
	validate_action_modes(&spec);
	validate_action_capabilities(&spec);

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize actions spec");
	ctx.write_blob("actions.bin", &bin);
}

fn validate_action_modes(spec: &ActionsSpec) {
	for mode in spec
		.actions
		.iter()
		.flat_map(|action| action.bindings.iter().map(|binding| binding.mode.as_str()))
		.chain(spec.prefixes.iter().map(|prefix| prefix.mode.as_str()))
	{
		if !VALID_MODES.contains(&mode) {
			panic!("unknown action mode: '{mode}'");
		}
	}
}

fn validate_action_capabilities(spec: &ActionsSpec) {
	for cap in spec.actions.iter().flat_map(|action| action.common.caps.iter().map(String::as_str)) {
		if !VALID_CAPS.contains(&cap) {
			panic!("unknown action capability: '{cap}'");
		}
	}
}
