use futures::future::LocalBoxFuture;
use xeno_registry_notifications::keys;

use crate::{CommandContext, CommandError, CommandOutcome, command};

command!(edit, { aliases: &["e"], description: "Edit a file" }, handler: cmd_edit);

/// Handler for the `:edit` command.
fn cmd_edit<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			return Err(CommandError::MissingArgument("filename"));
		}
		ctx.emit(keys::not_implemented(&format!("edit {}", ctx.args[0])));
		Ok(CommandOutcome::Ok)
	})
}
