#[path = "src/defs/loader.rs"]
mod defs_loader;
#[path = "src/schema/mod.rs"]
mod schema;
#[path = "build_support/mod.rs"]
mod build_support;

use build_support::compile::BuildCtx;

fn main() {
	let ctx = BuildCtx::new();

	build_support::actions::build(&ctx);
	build_support::grammars::build(&ctx);
	build_support::languages::build(&ctx);
	build_support::lsp_servers::build(&ctx);
	build_support::commands::build(&ctx);
	build_support::motions::build(&ctx);
	build_support::textobj::build(&ctx);
	build_support::options::build(&ctx);
	build_support::gutters::build(&ctx);
	build_support::statusline::build(&ctx);
	build_support::hooks::build(&ctx);
	build_support::notifications::build(&ctx);
	build_support::snippets::build(&ctx);
	build_support::themes::build(&ctx);
	build_support::keymaps::build(&ctx);
}
