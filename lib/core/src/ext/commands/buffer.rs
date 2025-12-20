use crate::command;
use crate::ext::{CommandContext, CommandError, CommandOutcome};

command!(buffer, &["b"], "Switch to buffer", handler: cmd_buffer);

fn cmd_buffer(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if ctx.args.is_empty() {
		return Err(CommandError::MissingArgument("buffer name or number"));
	}
	ctx.message(&format!("buffer {} - not yet implemented", ctx.args[0]));
	Ok(CommandOutcome::Ok)
}

command!(buffer_next, &["bn"], "Go to next buffer", handler: cmd_buffer_next);

fn cmd_buffer_next(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.message("buffer-next - not yet implemented");
	Ok(CommandOutcome::Ok)
}

command!(buffer_prev, &["bp"], "Go to previous buffer", handler: cmd_buffer_prev);

fn cmd_buffer_prev(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.message("buffer-previous - not yet implemented");
	Ok(CommandOutcome::Ok)
}

command!(delete_buffer, &["db"], "Delete current buffer", handler: cmd_delete_buffer);

fn cmd_delete_buffer(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.message("delete-buffer - not yet implemented");
	Ok(CommandOutcome::Ok)
}
