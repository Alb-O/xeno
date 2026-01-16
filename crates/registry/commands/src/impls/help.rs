use futures::future::LocalBoxFuture;
use xeno_registry_notifications::keys;

use crate::{CommandContext, CommandError, CommandOutcome, all_commands, command, find_command};

command!(help, { aliases: &["h"], description: "Show help for commands" }, handler: cmd_help);

/// Handler for the `:help` command.
fn cmd_help<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if let Some(cmd_name) = ctx.args.first() {
			if let Some(cmd) = find_command(cmd_name) {
				let mut out = Vec::new();
				out.push(format!("Command: :{}", cmd.name()));
				if !cmd.aliases().is_empty() {
					out.push(format!("Aliases: {}", cmd.aliases().join(", ")));
				}
				out.push(format!("Description: {}", cmd.description()));
				out.push(format!("Source: {}", cmd.source()));
				out.push(format!("Priority: {}", cmd.priority()));
				if !cmd.required_caps().is_empty() {
					let caps: Vec<String> = cmd
						.required_caps()
						.iter()
						.map(|c| format!("{c:?}"))
						.collect();
					out.push(format!("Required Capabilities: {}", caps.join(", ")));
				}
				ctx.emit(keys::help_text::call(out.join("\n")));
				return Ok(CommandOutcome::Ok);
			} else {
				return Err(CommandError::NotFound(cmd_name.to_string()));
			}
		}

		let mut sorted_commands: Vec<_> = all_commands().collect();
		sorted_commands.sort_by_key(|c| c.name());

		let help_text: Vec<String> = sorted_commands
			.iter()
			.map(|c| {
				let aliases = if c.aliases().is_empty() {
					String::new()
				} else {
					format!(" ({})", c.aliases().join(", "))
				};
				format!(":{}{} - {}", c.name(), aliases, c.description())
			})
			.collect();
		ctx.emit(keys::help_text::call(help_text.join(" | ")));
		Ok(CommandOutcome::Ok)
	})
}
