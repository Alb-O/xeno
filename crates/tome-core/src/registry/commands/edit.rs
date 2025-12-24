use crate::command;
use crate::registry::{CommandContext, CommandError, CommandOutcome};

command!(edit, { aliases: &["e"], description: "Edit a file" }, handler: cmd_edit);

fn cmd_edit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if ctx.args.is_empty() {
		return Err(CommandError::MissingArgument("filename"));
	}
	ctx.message(&format!("edit {} - not yet implemented", ctx.args[0]));
	Ok(CommandOutcome::Ok)
}
