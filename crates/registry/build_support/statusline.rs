//! NUON → [`StatuslineSpec`] compiler.

use std::collections::HashSet;

use crate::schema::statusline::{StatuslineSpec, VALID_POSITIONS};
use crate::build_support::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/statusline/assets/statusline.nuon");
	ctx.rerun_if_changed(&path);

	let spec: StatuslineSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for seg in &spec.segments {
		let name = &seg.common.name;
		if !seen.insert(name) {
			panic!("duplicate statusline segment name: '{name}'");
		}
		assert!(
			VALID_POSITIONS.contains(&seg.position.as_str()),
			"segment '{name}': unknown position '{}'",
			seg.position
		);
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize statusline spec");
	ctx.write_blob("statusline.bin", &bin);
}
