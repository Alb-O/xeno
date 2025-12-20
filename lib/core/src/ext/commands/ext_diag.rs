use crate::command;
use crate::ext::index::get_registry;
use crate::ext::{CommandContext, CommandError, CommandOutcome};

command!(ext_diag, &["ext.diag"], "Show extension system diagnostics", handler: cmd_ext_diag);

fn cmd_ext_diag(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	let reg = get_registry();
	let mut out = Vec::new();

	out.push("--- Commands ---".to_string());
	out.push(format!("Total: {}", reg.commands.by_name.len()));
	if !reg.commands.collisions.is_empty() {
		out.push(format!("Collisions: {}", reg.commands.collisions.len()));
		for c in &reg.commands.collisions {
			out.push(format!(
				"  {} collision on '{}': {} shadowed by {}",
				c.source, c.key, c.second_id, c.first_id
			));
		}
	}

	out.push("\n--- Actions ---".to_string());
	out.push(format!("Total: {}", reg.actions.by_name.len()));
	if !reg.actions.collisions.is_empty() {
		out.push(format!("Collisions: {}", reg.actions.collisions.len()));
		for c in &reg.actions.collisions {
			out.push(format!(
				"  {} collision on '{}': {} shadowed by {}",
				c.source, c.key, c.second_id, c.first_id
			));
		}
	}

	ctx.message(&out.join("\n"));
	Ok(CommandOutcome::Ok)
}
