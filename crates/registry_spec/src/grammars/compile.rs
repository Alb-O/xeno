//! NUON → [`GrammarsSpec`] compiler.

use std::collections::HashSet;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let root = ctx.asset("src/domains/grammars/assets");
	ctx.rerun_tree(&root);

	let path = root.join("grammars.nuon");
	let spec: GrammarsSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for grammar in &spec.grammars {
		if !seen.insert(&grammar.id) {
			panic!("duplicate grammar id: '{}'", grammar.id);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize grammars spec");
	ctx.write_blob("grammars.bin", &bin);
}
