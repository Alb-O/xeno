//! NUON → [`CommandsSpec`] compiler.

use std::collections::HashSet;

use crate::build_support::compile::*;
use crate::schema::commands::CommandsSpec;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/commands/assets/commands.nuon");
	ctx.rerun_if_changed(&path);

	let spec: CommandsSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for cmd in &spec.commands {
		if !seen.insert(&cmd.common.name) {
			panic!("duplicate command name: '{}'", cmd.common.name);
		}

		let mut seen_optional = false;
		let mut variadic_count = 0usize;
		for (idx, arg) in cmd.palette.args.iter().enumerate() {
			if !arg.required {
				seen_optional = true;
			} else if seen_optional {
				panic!("command '{}' has required arg '{}' after optional args", cmd.common.name, arg.name);
			}

			if arg.variadic {
				variadic_count += 1;
				if idx + 1 != cmd.palette.args.len() {
					panic!("command '{}' arg '{}' is variadic but not last", cmd.common.name, arg.name);
				}
			}
		}
		if variadic_count > 1 {
			panic!("command '{}' has multiple variadic args; only one variadic arg is supported", cmd.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize commands spec");
	ctx.write_blob("commands.bin", &bin);
}
