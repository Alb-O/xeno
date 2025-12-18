use std::path::PathBuf;

use linkme::distributed_slice;

use crate::ext::{COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome};

#[distributed_slice(COMMANDS)]
static CMD_WRITE: CommandDef = CommandDef {
	name: "write",
	aliases: &["w"],
	description: "Write buffer to file",
	handler: cmd_write,
};

fn cmd_write(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if let Some(&filename) = ctx.args.first() {
		ctx.editor.save_as(PathBuf::from(filename))?;
	} else {
		ctx.editor.save()?;
	}
	Ok(CommandOutcome::Ok)
}

#[distributed_slice(COMMANDS)]
static CMD_WRITE_QUIT: CommandDef = CommandDef {
	name: "wq",
	aliases: &["x"],
	description: "Write and quit",
	handler: cmd_write_quit,
};

fn cmd_write_quit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if let Some(&filename) = ctx.args.first() {
		ctx.editor.save_as(PathBuf::from(filename))?;
	} else {
		ctx.editor.save()?;
	}
	Ok(CommandOutcome::Quit)
}
