use futures::future::LocalBoxFuture;

use crate::command;
use crate::commands::{CommandContext, CommandError, CommandOutcome};
use crate::notifications::keys;

command!(edit, { aliases: &["e"], description: "Edit a file" }, handler: cmd_edit);

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

pub const DEFS: &[&crate::commands::CommandDef] = &[
	&CMD_edit,
];
