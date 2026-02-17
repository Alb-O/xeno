use std::path::PathBuf;

use xeno_primitives::BoxFutureLocal;

use crate::command_handler;
use crate::commands::{CommandContext, CommandError, CommandOutcome};

command_handler!(edit, handler: cmd_edit);

fn cmd_edit<'a>(ctx: &'a mut CommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			ctx.editor.defer_command("files".to_string(), Vec::new());
			return Ok(CommandOutcome::Ok);
		}
		let path = PathBuf::from(ctx.args[0]);
		ctx.editor.goto_file(path, 0, 0).await?;
		Ok(CommandOutcome::Ok)
	})
}
