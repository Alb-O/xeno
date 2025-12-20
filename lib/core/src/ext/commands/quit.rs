use crate::command;
use crate::ext::{CommandContext, CommandError, CommandOutcome};

command!(quit, &["q"], "Quit the editor", handler: cmd_quit);

fn cmd_quit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if ctx.editor.is_modified() {
		ctx.error("Buffer has unsaved changes (use :q! to force quit)");
		return Ok(CommandOutcome::Ok);
	}
	Ok(CommandOutcome::Quit)
}

command!(quit_force, &["q!"], "Quit without saving", handler: cmd_quit_force);

fn cmd_quit_force(_ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	Ok(CommandOutcome::ForceQuit)
}
