//! NUON → [`CommandsSpec`] compiler.

use std::collections::HashSet;

use super::CommandsSpec;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/commands/assets/commands.nuon");
	ctx.rerun_if_changed(&path);

	let spec: CommandsSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for cmd in &spec.commands {
		if !seen.insert(&cmd.common.name) {
			panic!("duplicate command name: '{}'", cmd.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize commands spec");
	ctx.write_blob("commands.bin", &bin);
}
