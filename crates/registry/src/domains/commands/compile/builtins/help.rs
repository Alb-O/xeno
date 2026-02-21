use xeno_primitives::BoxFutureLocal;

use crate::command_handler;
use crate::commands::{CommandContext, CommandError, CommandOutcome, RegistryEntry, all_commands, find_command};
use crate::notifications::keys;

command_handler!(help, handler: cmd_help);

fn cmd_help<'a>(ctx: &'a mut CommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if let Some(cmd_name) = ctx.args.first() {
			if let Some(cmd) = find_command(cmd_name) {
				let mut out = Vec::new();
				out.push(format!("Command: :{}", cmd.name_str()));
				let keyes = cmd.keys_resolved();
				if !keyes.is_empty() {
					out.push(format!("Secondary Keys: {}", keyes.join(", ")));
				}
				out.push(format!("Description: {}", cmd.description_str()));
				out.push(format!("Source: {}", cmd.source()));
				out.push(format!("Priority: {}", cmd.priority()));
				if cmd.mutates_buffer() {
					out.push("Mutates Buffer: yes".to_string());
				}
				ctx.emit(keys::help_text(out.join("\n")));
				return Ok(CommandOutcome::Ok);
			} else {
				return Err(CommandError::NotFound(cmd_name.to_string()));
			}
		}

		let mut sorted_commands = all_commands();
		sorted_commands.sort_by(|a, b| a.name_str().cmp(b.name_str()));

		let help_text: Vec<String> = sorted_commands
			.iter()
			.map(|c| {
				let keyes = c.keys_resolved();
				let key_str = if keyes.is_empty() {
					String::new()
				} else {
					format!(" ({})", keyes.join(", "))
				};
				format!(":{}{} - {}", c.name_str(), key_str, c.description_str())
			})
			.collect();
		ctx.emit(keys::help_text(help_text.join(" | ")));
		Ok(CommandOutcome::Ok)
	})
}
