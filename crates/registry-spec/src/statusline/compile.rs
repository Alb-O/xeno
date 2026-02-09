//! KDL → [`StatuslineSpec`] compiler.

use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/statusline/assets/statusline.kdl");
	ctx.rerun_if_changed(&path);

	let kdl = fs::read_to_string(&path).expect("failed to read statusline.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse statusline.kdl");

	let mut segments = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"segment",
			"unexpected top-level node '{}' in statusline.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "segment");
		let context = format!("segment '{name}'");
		let description = require_str(node, "description", &context);
		let keys = collect_keys(node);
		let short_desc = node
			.get("short-desc")
			.and_then(|v| v.as_string())
			.map(String::from);
		let position = require_str(node, "position", &context);
		assert!(
			VALID_POSITIONS.contains(&position.as_str()),
			"{context}: unknown position '{position}'"
		);
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
			panic!("{context}: statusline segments do not support 'caps'");
		}

		segments.push(StatuslineSegmentSpec {
			common: MetaCommonSpec {
				name,
				description,
				short_desc,
				keys,
				priority,
				caps: vec![],
				flags,
			},
			position,
		});
	}

	let pairs: Vec<(String, String)> = segments
		.iter()
		.map(|s| (s.common.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "segment");

	let spec = StatuslineSpec { segments };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize statusline spec");
	ctx.write_blob("statusline.bin", &bin);
}
