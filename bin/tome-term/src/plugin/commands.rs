use linkme::distributed_slice;
use tome_core::ext::{COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome};

#[distributed_slice(COMMANDS)]
static CMD_PLUGINS: CommandDef = CommandDef {
	name: "plugins",
	aliases: &["plugin"],
	description: "Manage plugins",
	handler: cmd_plugins,
};

fn cmd_plugins(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	match ctx.editor.plugin_command(ctx.args) {
		Ok(()) => Ok(CommandOutcome::Ok),
		Err(e) => Err(CommandError::Failed(e)),
	}
}
