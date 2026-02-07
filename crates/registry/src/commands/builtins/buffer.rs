use xeno_primitives::BoxFutureLocal;

use crate::command_handler;
use crate::commands::{CommandContext, CommandError, CommandOutcome};
use crate::notifications::keys;

command_handler!(buffer, handler: cmd_buffer);

fn cmd_buffer<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			return Err(CommandError::MissingArgument("buffer name or number"));
		}
		ctx.emit(keys::not_implemented(&format!("buffer {}", ctx.args[0])));
		Ok(CommandOutcome::Ok)
	})
}

command_handler!(buffer_next, handler: cmd_buffer_next);

fn cmd_buffer_next<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.emit(keys::not_implemented("buffer-next"));
		Ok(CommandOutcome::Ok)
	})
}

command_handler!(buffer_prev, handler: cmd_buffer_prev);

fn cmd_buffer_prev<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.emit(keys::not_implemented("buffer-previous"));
		Ok(CommandOutcome::Ok)
	})
}

command_handler!(delete_buffer, handler: cmd_delete_buffer);

fn cmd_delete_buffer<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.emit(keys::not_implemented("delete-buffer"));
		Ok(CommandOutcome::Ok)
	})
}

command_handler!(readonly, handler: cmd_readonly);

fn cmd_readonly<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let current = ctx.is_readonly();
		ctx.set_readonly(!current);
		if !current {
			ctx.emit(keys::READONLY_ENABLED);
		} else {
			ctx.emit(keys::READONLY_DISABLED);
		}
		Ok(CommandOutcome::Ok)
	})
}
