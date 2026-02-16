//! NUON → [`ActionsSpec`] compiler.

use std::collections::HashSet;

use super::ActionsSpec;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/actions/assets/actions.nuon");
	ctx.rerun_if_changed(&path);

	let spec: ActionsSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for action in &spec.actions {
		if !seen.insert(&action.common.name) {
			panic!("duplicate action name: '{}'", action.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize actions spec");
	ctx.write_blob("actions.bin", &bin);
}
