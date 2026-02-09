//! KDL → [`OptionsSpec`] compiler.

use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/options/assets/options.kdl");
	ctx.rerun_if_changed(&path);

	let kdl = fs::read_to_string(&path).expect("failed to read options.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse options.kdl");

	let mut options = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"option",
			"unexpected top-level node '{}' in options.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "option");
		let context = format!("option '{name}'");

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
			panic!("{context}: options do not support 'caps'");
		}

		let kdl_key = require_str(node, "kdl-key", &context);
		let value_type = require_str(node, "value-type", &context);
		assert!(
			VALID_TYPES.contains(&value_type.as_str()),
			"{context}: unknown value-type '{value_type}'"
		);
		let scope = require_str(node, "scope", &context);
		assert!(
			VALID_SCOPES.contains(&scope.as_str()),
			"{context}: unknown scope '{scope}'"
		);
		let description = require_str(node, "description", &context);

		let default = require_str(node, "default", &context);

		let validator = node
			.get("validator")
			.and_then(|v| v.as_string())
			.map(String::from);

		options.push(OptionSpec {
			common: MetaCommonSpec {
				name,
				description,
				short_desc,
				keys,
				priority,
				caps: vec![],
				flags,
			},
			kdl_key,
			value_type,
			default,
			scope,
			validator,
		});
	}

	let pairs: Vec<(String, String)> = options
		.iter()
		.map(|o| (o.common.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "option");

	let spec = OptionsSpec { options };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize options spec");
	ctx.write_blob("options.bin", &bin);
}
