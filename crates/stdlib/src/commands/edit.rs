use futures::future::LocalBoxFuture;
use evildoer_manifest::{CommandContext, CommandError, CommandOutcome};

use crate::{NotifyWARNExt, command};

command!(edit, { aliases: &["e"], description: "Edit a file" }, handler: cmd_edit);

fn cmd_edit<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			return Err(CommandError::MissingArgument("filename"));
		}
		ctx.warn(&format!("edit {} - not yet implemented", ctx.args[0]));
		Ok(CommandOutcome::Ok)
	})
}
