use std::path::PathBuf;

use crate::command;
use crate::ext::{CommandContext, CommandError, CommandOutcome};

command!(write, &["w"], "Write buffer to file", handler: cmd_write);

fn cmd_write(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if let Some(&filename) = ctx.args.first() {
		ctx.editor.save_as(PathBuf::from(filename))?;
	} else {
		ctx.editor.save()?;
	}
	Ok(CommandOutcome::Ok)
}

command!(wq, &["x"], "Write and quit", handler: cmd_write_quit);

fn cmd_write_quit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if let Some(&filename) = ctx.args.first() {
		ctx.editor.save_as(PathBuf::from(filename))?;
	} else {
		ctx.editor.save()?;
	}
	Ok(CommandOutcome::Quit)
}
