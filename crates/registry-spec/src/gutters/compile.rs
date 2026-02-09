//! KDL → [`GuttersSpec`] compiler.

use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/gutter/assets/gutters.kdl");
	ctx.rerun_if_changed(&path);

	let kdl = fs::read_to_string(&path).expect("failed to read gutters.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse gutters.kdl");

	let mut gutters = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"gutter",
			"unexpected top-level node '{}' in gutters.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "gutter");
		let context = format!("gutter '{name}'");
		let description = require_str(node, "description", &context);
		let keys = collect_keys(node);
		let short_desc = node
			.get("short-desc")
			.and_then(|v| v.as_string())
			.map(String::from);
		let priority = node
			.get("priority")
			.and_then(|v| v.as_integer())
			.map(|v| v as i16)
			.unwrap_or(0);
		let flags = node
			.get("flags")
			.and_then(|v| v.as_integer())
			.map(|v| v as u32)
			.unwrap_or(0);
		if let Some(children) = node.children()
			&& children.get("caps").is_some()
		{
			panic!("{context}: gutters do not support 'caps'");
		}

		let width = node
			.get("width")
			.map(|v| {
				if let Some(s) = v.as_string() {
					s.to_string()
				} else if let Some(i) = v.as_integer() {
					i.to_string()
				} else {
					panic!("{context}: width must be 'dynamic' or integer");
				}
			})
			.unwrap_or_else(|| "dynamic".to_string());

		let enabled = node
			.get("enabled")
			.and_then(|v| v.as_bool())
			.unwrap_or(true);

		gutters.push(GutterSpec {
			common: MetaCommonSpec {
				name,
				description,
				short_desc,
				keys,
				priority,
				caps: vec![],
				flags,
			},
			width,
			enabled,
		});
	}

	let pairs: Vec<(String, String)> = gutters
		.iter()
		.map(|g| (g.common.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "gutter");

	let spec = GuttersSpec { gutters };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize gutters spec");
	ctx.write_blob("gutters.bin", &bin);
}
