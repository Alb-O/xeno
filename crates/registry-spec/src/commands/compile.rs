//! KDL → [`CommandsSpec`] compiler.

use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/commands/assets/commands.kdl");
	ctx.rerun_if_changed(&path);

	let kdl = fs::read_to_string(&path).expect("failed to read commands.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse commands.kdl");

	let mut commands = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"command",
			"unexpected top-level node '{}' in commands.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "command");
		let context = format!("command '{name}'");
		let description = require_str(node, "description", &context);
		let keys = collect_keys(node);
		let short_desc = node.get("short-desc").and_then(|v| v.as_string()).map(String::from);
		let priority = node.get("priority").and_then(|v| v.as_integer()).map(|v| v as i16).unwrap_or(0);
		let flags = node.get("flags").and_then(|v| v.as_integer()).map(|v| v as u32).unwrap_or(0);
		if let Some(children) = node.children()
			&& children.get("caps").is_some()
		{
			panic!("{context}: commands do not support 'caps'");
		}
		commands.push(CommandSpec {
			common: MetaCommonSpec {
				name,
				description,
				short_desc,
				keys,
				priority,
				caps: vec![],
				flags,
			},
		});
	}

	let pairs: Vec<(String, String)> = commands.iter().map(|c| (c.common.name.clone(), String::new())).collect();
	validate_unique(&pairs, "command");

	let spec = CommandsSpec { commands };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize commands spec");
	ctx.write_blob("commands.bin", &bin);
}
