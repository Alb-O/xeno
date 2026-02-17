use xeno_registry_spec::compile::BuildCtx;

fn main() {
	let ctx = BuildCtx::new();

	xeno_registry_spec::actions::compile::build(&ctx);
	xeno_registry_spec::grammars::compile::build(&ctx);
	xeno_registry_spec::languages::compile::build(&ctx);
	xeno_registry_spec::lsp_servers::compile::build(&ctx);
	xeno_registry_spec::commands::compile::build(&ctx);
	xeno_registry_spec::motions::compile::build(&ctx);
	xeno_registry_spec::textobj::compile::build(&ctx);
	xeno_registry_spec::options::compile::build(&ctx);
	xeno_registry_spec::gutters::compile::build(&ctx);
	xeno_registry_spec::statusline::compile::build(&ctx);
	xeno_registry_spec::hooks::compile::build(&ctx);
	xeno_registry_spec::notifications::compile::build(&ctx);
	xeno_registry_spec::snippets::compile::build(&ctx);
	xeno_registry_spec::themes::compile::build(&ctx);
	xeno_registry_spec::keymaps::compile::build(&ctx);
}
