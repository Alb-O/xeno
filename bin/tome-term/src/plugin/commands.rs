use linkme::distributed_slice;
use tome_core::ext::{COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome};

#[distributed_slice(COMMANDS)]
static CMD_PLUGINS: CommandDef = CommandDef {
	name: "plugins",
	aliases: &["plugin"],
	description: "Manage plugins",
	handler: cmd_plugins,
	user_data: None,
};

fn cmd_plugins(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.editor.plugin_command(ctx.args).map(|_| CommandOutcome::Ok)
}
