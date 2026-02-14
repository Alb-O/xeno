//! NUON → [`MotionsSpec`] compiler.

use std::collections::HashSet;

use super::MotionsSpec;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/motions/assets/motions.nuon");
	ctx.rerun_if_changed(&path);

	let spec: MotionsSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for motion in &spec.motions {
		if !seen.insert(&motion.common.name) {
			panic!("duplicate motion name: '{}'", motion.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize motions spec");
	ctx.write_blob("motions.bin", &bin);
}
