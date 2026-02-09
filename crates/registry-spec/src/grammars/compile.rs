//! KDL → [`GrammarsSpec`] compiler.

use std::collections::HashSet;
use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::BuildCtx;

pub fn build(ctx: &BuildCtx) {
	let root = ctx.asset("src/domains/grammars/assets");
	ctx.rerun_tree(&root);

	let path = root.join("grammars.kdl");
	let kdl = fs::read_to_string(&path).expect("failed to read grammars.kdl");
	let grammars = parse_grammars_kdl(&kdl);

	let mut seen = HashSet::new();
	for grammar in &grammars {
		if !seen.insert(&grammar.id) {
			panic!("duplicate grammar id: '{}'", grammar.id);
		}
	}

	let spec = GrammarsSpec { grammars };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize grammars spec");
	ctx.write_blob("grammars.bin", &bin);
}

fn parse_grammars_kdl(input: &str) -> Vec<GrammarSpec> {
	let doc: KdlDocument = input.parse().expect("failed to parse grammars.kdl");
	let mut grammars = Vec::new();

	for node in doc.nodes() {
		let id = node.name().value().to_string();

		let children = match node.children() {
			Some(c) => c,
			None => continue,
		};

		let source = if let Some(path_node) = children.get("path") {
			let path = path_node
				.entry(0)
				.and_then(|e| e.value().as_string())
				.unwrap_or_else(|| panic!("grammar '{}' path node missing value", id))
				.to_string();
			GrammarSourceSpec::Local { path }
		} else {
			let source_node = children
				.get("source")
				.unwrap_or_else(|| panic!("grammar '{}' missing 'source' or 'path' child", id));

			let remote = source_node
				.entry(0)
				.and_then(|e| e.value().as_string())
				.unwrap_or_else(|| panic!("grammar '{}' source node missing URL value", id))
				.to_string();

			let rev_node = children
				.get("rev")
				.unwrap_or_else(|| panic!("grammar '{}' missing 'rev' child", id));

			let revision = rev_node
				.entry(0)
				.and_then(|e| e.value().as_string())
				.unwrap_or_else(|| panic!("grammar '{}' rev node missing value", id))
				.to_string();

			let subpath = children
				.get("subpath")
				.and_then(|n| n.entry(0))
				.and_then(|e| e.value().as_string())
				.map(|s| s.to_string());

			GrammarSourceSpec::Git {
				remote,
				revision,
				subpath,
			}
		};

		grammars.push(GrammarSpec { id, source });
	}

	grammars.sort_by(|a, b| a.id.cmp(&b.id));
	grammars
}
