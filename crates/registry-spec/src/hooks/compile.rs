//! KDL → [`HooksSpec`] compiler.

use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/hooks/assets/hooks.kdl");
	ctx.rerun_if_changed(&path);

	let kdl = fs::read_to_string(&path).expect("failed to read hooks.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse hooks.kdl");

	let mut hooks = Vec::new();
	for node in doc.nodes() {
		assert_eq!(node.name().value(), "hook", "unexpected top-level node '{}' in hooks.kdl", node.name().value());
		let name = node_name_arg(node, "hook");
		let context = format!("hook '{name}'");
		let event = require_str(node, "event", &context);
		let description = require_str(node, "description", &context);
		let keys = collect_keys(node);
		let short_desc = node.get("short-desc").and_then(|v| v.as_string()).map(String::from);
		let priority = node.get("priority").and_then(|v| v.as_integer()).map(|v| v as i16).unwrap_or(0);
		let flags = node.get("flags").and_then(|v| v.as_integer()).map(|v| v as u32).unwrap_or(0);
		if let Some(children) = node.children()
			&& children.get("caps").is_some()
		{
			panic!("{context}: hooks do not support 'caps'");
		}

		hooks.push(HookSpec {
			common: MetaCommonSpec {
				name,
				description,
				short_desc,
				keys,
				priority,
				caps: vec![],
				flags,
			},
			event,
		});
	}

	let pairs: Vec<(String, String)> = hooks.iter().map(|h| (h.common.name.clone(), String::new())).collect();
	validate_unique(&pairs, "hook");

	let spec = HooksSpec { hooks };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize hooks spec");
	ctx.write_blob("hooks.bin", &bin);
}
