//! NUON → [`OptionsSpec`] compiler.

use std::collections::HashSet;

use crate::build_support::compile::*;
use crate::schema::options::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/options/assets/options.nuon");
	ctx.rerun_if_changed(&path);

	let spec: OptionsSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for opt in &spec.options {
		assert!(
			VALID_TYPES.contains(&opt.value_type.as_str()),
			"option '{}': unknown value_type '{}'",
			opt.common.name,
			opt.value_type
		);
		assert!(
			VALID_SCOPES.contains(&opt.scope.as_str()),
			"option '{}': unknown scope '{}'",
			opt.common.name,
			opt.scope
		);
		if !seen.insert(&opt.common.name) {
			panic!("duplicate option name: '{}'", opt.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize options spec");
	ctx.write_blob("options.bin", &bin);
}
