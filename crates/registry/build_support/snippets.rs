//! NUON → [`SnippetsSpec`] compiler.

use std::collections::HashSet;

use crate::schema::snippets::{SnippetSpec, SnippetsSpec};
use crate::build_support::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/snippets/assets/snippets.nuon");
	ctx.rerun_if_changed(&path);

	let mut spec: SnippetsSpec = read_nuon_spec(&path);

	for snippet in &mut spec.snippets {
		snippet.common.name = normalize_lookup_key(&snippet.common.name);
		snippet.common.keys = snippet.common.keys.iter().map(|k| normalize_lookup_key(k)).collect();
	}

	validate_lookup_uniqueness(&spec.snippets);

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize snippets spec");
	ctx.write_blob("snippets.bin", &bin);
}

fn normalize_lookup_key(raw: &str) -> String {
	let normalized = raw.strip_prefix('@').unwrap_or(raw).trim();
	if normalized.is_empty() {
		panic!("snippet lookup key must not be empty");
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
