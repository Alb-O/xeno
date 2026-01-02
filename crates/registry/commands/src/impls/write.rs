use std::path::PathBuf;

use futures::future::LocalBoxFuture;

use crate::{CommandContext, CommandError, CommandOutcome, command};

command!(write, { aliases: &["w"], description: "Write buffer to file" }, handler: cmd_write);

/// Handler for the `:write` command.
fn cmd_write<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if let Some(&filename) = ctx.args.first() {
			ctx.editor.save_as(PathBuf::from(filename)).await?;
		} else {
			ctx.editor.save().await?;
		}
		Ok(CommandOutcome::Ok)
	})
}

command!(wq, { aliases: &["x"], description: "Write and quit" }, handler: cmd_write_quit);

/// Handler for the `:wq` command.
fn cmd_write_quit<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if let Some(&filename) = ctx.args.first() {
			ctx.editor.save_as(PathBuf::from(filename)).await?;
		} else {
			ctx.editor.save().await?;
		}
		Ok(CommandOutcome::Quit)
	})
}
