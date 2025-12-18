use linkme::distributed_slice;

use crate::ext::{COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome};

#[distributed_slice(COMMANDS)]
static CMD_QUIT: CommandDef = CommandDef {
	name: "quit",
	aliases: &["q"],
	description: "Quit the editor",
	handler: cmd_quit,
};

fn cmd_quit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if ctx.editor.is_modified() {
		ctx.error("Buffer has unsaved changes (use :q! to force quit)");
		return Ok(CommandOutcome::Ok);
	}
	Ok(CommandOutcome::Quit)
}

#[distributed_slice(COMMANDS)]
static CMD_QUIT_FORCE: CommandDef = CommandDef {
	name: "quit!",
	aliases: &["q!"],
	description: "Quit without saving",
	handler: cmd_quit_force,
};

fn cmd_quit_force(_ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	Ok(CommandOutcome::ForceQuit)
}
