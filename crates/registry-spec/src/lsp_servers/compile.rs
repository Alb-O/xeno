//! NUON → [`LspServersSpec`] compiler.

use std::collections::HashSet;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let root = ctx.asset("src/domains/lsp_servers/assets");
	ctx.rerun_tree(&root);

	let path = root.join("lsp_servers.nuon");
	let spec: LspServersSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for server in &spec.servers {
		if !seen.insert(&server.common.name) {
			panic!("duplicate lsp server name: '{}'", server.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize lsp_servers spec");
	ctx.write_blob("lsp_servers.bin", &bin);
}
