//! KDL → [`TextObjectsSpec`] compiler.

use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/textobj/assets/text_objects.kdl");
	ctx.rerun_if_changed(&path);

	let kdl = fs::read_to_string(&path).expect("failed to read text_objects.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse text_objects.kdl");

	let mut text_objects = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"text_object",
			"unexpected top-level node '{}' in text_objects.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "text_object");
		let context = format!("text_object '{name}'");
		let description = require_str(node, "description", &context);
		let keys = collect_keys(node);
		let short_desc = node.get("short-desc").and_then(|v| v.as_string()).map(String::from);
		let priority = node.get("priority").and_then(|v| v.as_integer()).map(|v| v as i16).unwrap_or(0);
		let flags = node.get("flags").and_then(|v| v.as_integer()).map(|v| v as u32).unwrap_or(0);
		if let Some(children) = node.children()
			&& children.get("caps").is_some()
		{
			panic!("{context}: text objects do not support 'caps'");
		}
		let trigger = require_str(node, "trigger", &context);

		let alt_triggers = node
			.children()
			.and_then(|c| c.get("alt-triggers"))
			.map(|n| {
				n.entries()
					.iter()
					.filter(|e| e.name().is_none())
					.filter_map(|e| e.value().as_string().map(String::from))
					.collect()
			})
			.unwrap_or_default();

		text_objects.push(TextObjectSpec {
			common: MetaCommonSpec {
				name,
				description,
				short_desc,
				keys,
				priority,
				caps: vec![],
				flags,
			},
			trigger,
			alt_triggers,
		});
	}

	let pairs: Vec<(String, String)> = text_objects.iter().map(|t| (t.common.name.clone(), String::new())).collect();
	validate_unique(&pairs, "text_object");

	let spec = TextObjectsSpec { text_objects };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize text_objects spec");
	ctx.write_blob("text_objects.bin", &bin);
}
