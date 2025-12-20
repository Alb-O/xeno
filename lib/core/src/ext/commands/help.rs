use crate::command;
use crate::ext::{CommandContext, CommandError, CommandOutcome, find_command};

command!(help, { aliases: &["h"], description: "Show help for commands" }, handler: cmd_help);

fn cmd_help(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	if let Some(cmd_name) = ctx.args.first() {
		if let Some(cmd) = find_command(cmd_name) {
			let mut out = Vec::new();
			out.push(format!("Command: :{}", cmd.name));
			if !cmd.aliases.is_empty() {
				out.push(format!("Aliases: {}", cmd.aliases.join(", ")));
			}
			out.push(format!("Description: {}", cmd.description));
			out.push(format!("Source: {}", cmd.source));
			out.push(format!("Priority: {}", cmd.priority));
			if !cmd.required_caps.is_empty() {
				let caps: Vec<_> = cmd.required_caps.iter().map(|c| c.to_string()).collect();
				out.push(format!("Required Capabilities: {}", caps.join(", ")));
			}
			ctx.message(&out.join("\n"));
			return Ok(CommandOutcome::Ok);
		} else {
			return Err(CommandError::NotFound(cmd_name.to_string()));
		}
	}

	let mut sorted_commands: Vec<_> = crate::ext::all_commands().collect();
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
