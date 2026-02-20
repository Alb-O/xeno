//! NUON → [`TextObjectsSpec`] compiler.

use std::collections::HashSet;

use crate::schema::textobj::TextObjectsSpec;
use crate::build_support::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/textobj/assets/text_objects.nuon");
	ctx.rerun_if_changed(&path);

	let spec: TextObjectsSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for obj in &spec.text_objects {
		if !seen.insert(&obj.common.name) {
			panic!("duplicate text object name: '{}'", obj.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize text_objects spec");
	ctx.write_blob("text_objects.bin", &bin);
}
