use xeno_primitives::BoxFutureLocal;

use crate::command_handler;
use crate::commands::{CommandContext, CommandError, CommandOutcome};
use crate::notifications::keys;

command_handler!(edit, handler: cmd_edit);

fn cmd_edit<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			return Err(CommandError::MissingArgument("filename"));
		}
		ctx.emit(keys::not_implemented(&format!("edit {}", ctx.args[0])));
		Ok(CommandOutcome::Ok)
	})
}
