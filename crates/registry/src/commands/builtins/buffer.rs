use futures::future::LocalBoxFuture;

use crate::commands::{CommandContext, CommandError, CommandOutcome, command};
use crate::notifications::keys;

command!(buffer, { aliases: &["b"], description: "Switch to buffer" }, handler: cmd_buffer);

/// Handler for the `:buffer` command.
fn cmd_buffer<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			return Err(CommandError::MissingArgument("buffer name or number"));
		}
		ctx.emit(keys::not_implemented(&format!("buffer {}", ctx.args[0])));
		Ok(CommandOutcome::Ok)
	})
}

command!(
	buffer_next,
	{ aliases: &["bn"], description: "Go to next buffer" },
	handler: cmd_buffer_next
);

/// Handler for the `:buffer-next` command.
fn cmd_buffer_next<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.emit(keys::not_implemented("buffer-next"));
		Ok(CommandOutcome::Ok)
	})
}

command!(
	buffer_prev,
	{ aliases: &["bp"], description: "Go to previous buffer" },
	handler: cmd_buffer_prev
);

/// Handler for the `:buffer-prev` command.
fn cmd_buffer_prev<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.emit(keys::not_implemented("buffer-previous"));
		Ok(CommandOutcome::Ok)
	})
}

command!(
	delete_buffer,
	{ aliases: &["db"], description: "Delete current buffer" },
	handler: cmd_delete_buffer
);

/// Handler for the `:delete-buffer` command.
fn cmd_delete_buffer<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.emit(keys::not_implemented("delete-buffer"));
		Ok(CommandOutcome::Ok)
	})
}

command!(
	readonly,
	{ aliases: &["ro"], description: "Toggle read-only mode for current buffer" },
	handler: cmd_readonly
);

/// Handler for the `:readonly` command.
fn cmd_readonly<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
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
