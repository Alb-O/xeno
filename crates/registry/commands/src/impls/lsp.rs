//! LSP-related commands.

use futures::future::LocalBoxFuture;

use crate::{CommandContext, CommandError, CommandOutcome, command};

command!(
	hover,
	{ aliases: &["lsp-hover"], description: "Show hover information at cursor" },
	handler: cmd_hover
);

fn cmd_hover<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let content = ctx
			.editor
			.lsp_hover()
			.await
			.ok_or_else(|| CommandError::Failed("No hover information available".into()))?;

		ctx.editor.open_info_popup(&content, Some("markdown"));
		Ok(CommandOutcome::Ok)
	})
}

command!(
	gd,
	{ aliases: &["goto-definition", "lsp-definition"], description: "Go to definition" },
	handler: cmd_goto_definition
);

fn cmd_goto_definition<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.lsp_goto_definition().await?;
		Ok(CommandOutcome::Ok)
	})
}
