//! KDL -> [`SnippetsSpec`] compiler.

use std::collections::HashSet;
use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/snippets/assets/snippets.kdl");
	ctx.rerun_if_changed(&path);

	let kdl = fs::read_to_string(&path).expect("failed to read snippets.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse snippets.kdl");

	let mut snippets = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"snippet",
			"unexpected top-level node '{}' in snippets.kdl",
			node.name().value()
		);

		let name_raw = node_name_arg(node, "snippet");
		let context = format!("snippet '{name_raw}'");
		let name = normalize_lookup_key(&name_raw, &context, "name");
		let description = require_str(node, "description", &context);
		let body = require_str(node, "body", &context);

		let mut seen_keys = HashSet::new();
		let mut keys = Vec::new();
		for key in collect_keys(node) {
			let normalized = normalize_lookup_key(&key, &context, "key");
			if seen_keys.insert(normalized.clone()) {
				keys.push(normalized);
			}
		}

		let short_desc = node.get("short-desc").and_then(|v| v.as_string()).map(String::from);
		let priority = node.get("priority").and_then(|v| v.as_integer()).map(|v| v as i16).unwrap_or(0);
		let flags = node.get("flags").and_then(|v| v.as_integer()).map(|v| v as u32).unwrap_or(0);

		if let Some(children) = node.children()
			&& children.get("caps").is_some()
		{
			panic!("{context}: snippets do not support 'caps'");
		}

		snippets.push(SnippetSpec {
			common: MetaCommonSpec {
				name,
				description,
				short_desc,
				keys,
				priority,
				caps: vec![],
				flags,
			},
			body,
		});
	}

	validate_lookup_uniqueness(&snippets);

	let spec = SnippetsSpec { snippets };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize snippets spec");
	ctx.write_blob("snippets.bin", &bin);
}

fn normalize_lookup_key(raw: &str, context: &str, field: &str) -> String {
	let normalized = raw.strip_prefix('@').unwrap_or(raw).trim();
	if normalized.is_empty() {
		panic!("{context}: {field} must not be empty");
	}
	normalized.to_string()
}

fn validate_lookup_uniqueness(snippets: &[SnippetSpec]) {
	let mut seen = HashSet::new();
	for snippet in snippets {
		let name = snippet.common.name.as_str();
		if !seen.insert(name.to_string()) {
			panic!("duplicate snippet lookup key: '{name}'");
		}

		for key in &snippet.common.keys {
			if !seen.insert(key.clone()) {
				panic!("duplicate snippet lookup key: '{key}'");
			}
		}
	}
}
