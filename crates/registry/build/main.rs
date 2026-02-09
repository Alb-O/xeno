mod actions;
mod common;
mod grammars;
mod languages;
mod lsp_servers;
mod registry;
mod themes;

use common::BuildCtx;

fn main() {
	let ctx = BuildCtx::new();

	actions::build(&ctx);
	grammars::build(&ctx);
	languages::build(&ctx);
	lsp_servers::build(&ctx);
	registry::build_all(&ctx);
	themes::build(&ctx);
}
