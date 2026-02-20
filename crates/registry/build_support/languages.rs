//! NUON → [`LanguagesSpec`] compiler.
//!
//! Language definitions live in `languages.nuon`. Tree-sitter query files
//! (`.scm`) are discovered from the sibling `queries/` directory and merged
//! into each language's spec at build time.

use std::collections::HashSet;

use crate::schema::languages::*;
use crate::build_support::compile::*;

pub fn build(ctx: &BuildCtx) {
	let root = ctx.asset("src/domains/languages/assets");
	ctx.rerun_tree(&root);

	let path = root.join("languages.nuon");
	let mut spec: LanguagesSpec = read_nuon_spec(&path);

	// Merge .scm query files from queries/<lang_name>/*.scm
	let queries_root = root.join("queries");
	for lang in &mut spec.langs {
		let lang_dir = queries_root.join(&lang.common.name);
		if lang_dir.exists() {
			for path in collect_files_sorted(&lang_dir, "scm") {
				let kind = path.file_stem().unwrap().to_str().unwrap().to_string();
				let text = std::fs::read_to_string(&path).expect("failed to read query");
				lang.queries.push(LanguageQuerySpec { kind, text });
			}
			lang.queries.sort_by(|a, b| a.kind.cmp(&b.kind));
		}
	}

	let mut seen = HashSet::new();
	for lang in &spec.langs {
		if !seen.insert(&lang.common.name) {
			panic!("duplicate language name: '{}'", lang.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize languages spec");
	ctx.write_blob("languages.bin", &bin);
}
