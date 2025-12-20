use crate::command;
use crate::ext::{COMMANDS, CommandContext, CommandError, CommandOutcome};

command!(help, &["h"], "Show help for commands", handler: cmd_help);

fn cmd_help(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	let mut sorted_commands: Vec<_> = COMMANDS.iter().collect();
	sorted_commands.sort_by_key(|c| c.name);

	let help_text: Vec<String> = sorted_commands
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
