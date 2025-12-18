use linkme::distributed_slice;

use crate::ext::{COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome};

#[distributed_slice(COMMANDS)]
static CMD_EDIT: CommandDef = CommandDef {
	name: "edit",
	aliases: &["e"],
	description: "Edit a file",
	handler: cmd_edit,
};

fn cmd_edit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if ctx.args.is_empty() {
		return Err(CommandError::MissingArgument("filename"));
	}
	ctx.message(&format!("edit {} - not yet implemented", ctx.args[0]));
	Ok(CommandOutcome::Ok)
}
