//! NUON → [`GuttersSpec`] compiler.

use std::collections::HashSet;

use super::GuttersSpec;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/gutter/assets/gutters.nuon");
	ctx.rerun_if_changed(&path);

	let spec: GuttersSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for gutter in &spec.gutters {
		if !seen.insert(&gutter.common.name) {
			panic!("duplicate gutter name: '{}'", gutter.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize gutters spec");
	ctx.write_blob("gutters.bin", &bin);
}
