use linkme::distributed_slice;

use crate::ext::{COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome};

#[distributed_slice(COMMANDS)]
static CMD_HELP: CommandDef = CommandDef {
	name: "help",
	aliases: &["h"],
	description: "Show help for commands",
	handler: cmd_help,
};

fn cmd_help(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	let help_text: Vec<String> = COMMANDS
		.iter()
		.map(|c| {
			let aliases = if c.aliases.is_empty() {
				String::new()
			} else {
				format!(" ({})", c.aliases.join(", "))
			};
			format!(":{}{} - {}", c.name, aliases, c.description)
		})
		.collect();
	ctx.message(&help_text.join(" | "));
	Ok(CommandOutcome::Ok)
}
