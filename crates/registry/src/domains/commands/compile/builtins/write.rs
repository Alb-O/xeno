use std::path::PathBuf;

use xeno_primitives::BoxFutureLocal;

use crate::command_handler;
use crate::commands::{CommandContext, CommandError, CommandOutcome};

command_handler!(write, handler: cmd_write);

fn cmd_write<'a>(ctx: &'a mut CommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if let Some(&filename) = ctx.args.first() {
			ctx.editor.save_as(PathBuf::from(filename)).await?;
		} else {
			ctx.editor.save().await?;
		}
		Ok(CommandOutcome::Ok)
	})
}

command_handler!(wq, handler: cmd_write_quit);

fn cmd_write_quit<'a>(ctx: &'a mut CommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if let Some(&filename) = ctx.args.first() {
			ctx.editor.save_as(PathBuf::from(filename)).await?;
		} else {
			ctx.editor.save().await?;
		}
		Ok(CommandOutcome::Quit)
	})
}
