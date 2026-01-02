use futures::future::LocalBoxFuture;

use crate::{CommandContext, CommandError, CommandOutcome, command};

command!(buffer, { aliases: &["b"], description: "Switch to buffer" }, handler: cmd_buffer);

/// Handler for the `:buffer` command.
fn cmd_buffer<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			return Err(CommandError::MissingArgument("buffer name or number"));
		}
		ctx.warn(&format!("buffer {} - not yet implemented", ctx.args[0]));
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
		ctx.warn("buffer-next - not yet implemented");
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
		ctx.warn("buffer-previous - not yet implemented");
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
		ctx.warn("delete-buffer - not yet implemented");
		Ok(CommandOutcome::Ok)
	})
}
