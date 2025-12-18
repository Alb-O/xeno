use linkme::distributed_slice;

use crate::ext::{COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome};

#[distributed_slice(COMMANDS)]
static CMD_BUFFER: CommandDef = CommandDef {
	name: "buffer",
	aliases: &["b"],
	description: "Switch to buffer",
	handler: cmd_buffer,
};

fn cmd_buffer(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if ctx.args.is_empty() {
		return Err(CommandError::MissingArgument("buffer name or number"));
	}
	ctx.message(&format!("buffer {} - not yet implemented", ctx.args[0]));
	Ok(CommandOutcome::Ok)
}

#[distributed_slice(COMMANDS)]
static CMD_BUFFER_NEXT: CommandDef = CommandDef {
	name: "buffer-next",
	aliases: &["bn"],
	description: "Go to next buffer",
	handler: cmd_buffer_next,
};

fn cmd_buffer_next(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.message("buffer-next - not yet implemented");
	Ok(CommandOutcome::Ok)
}

#[distributed_slice(COMMANDS)]
static CMD_BUFFER_PREV: CommandDef = CommandDef {
	name: "buffer-previous",
	aliases: &["bp"],
	description: "Go to previous buffer",
	handler: cmd_buffer_prev,
};

fn cmd_buffer_prev(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.message("buffer-previous - not yet implemented");
	Ok(CommandOutcome::Ok)
}

#[distributed_slice(COMMANDS)]
static CMD_DELETE_BUFFER: CommandDef = CommandDef {
	name: "delete-buffer",
	aliases: &["db"],
	description: "Delete current buffer",
	handler: cmd_delete_buffer,
};

fn cmd_delete_buffer(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.message("delete-buffer - not yet implemented");
	Ok(CommandOutcome::Ok)
}
