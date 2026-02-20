//! NUON → [`HooksSpec`] compiler.

use std::collections::HashSet;

use crate::build_support::compile::*;
use crate::schema::hooks::HooksSpec;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/hooks/assets/hooks.nuon");
	ctx.rerun_if_changed(&path);

	let spec: HooksSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for hook in &spec.hooks {
		if !seen.insert(&hook.common.name) {
			panic!("duplicate hook name: '{}'", hook.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize hooks spec");
	ctx.write_blob("hooks.bin", &bin);
}
