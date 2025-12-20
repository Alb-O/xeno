use crate::command;
use crate::ext::index::diagnostics;
use crate::ext::{CommandContext, CommandError, CommandOutcome};

command!(ext_diag, { aliases: &["ext.diag"], description: "Show extension system diagnostics" }, handler: cmd_ext_diag);
command!(ext_doctor, { aliases: &["ext.doctor"], description: "Check for extension collisions and suggest fixes" }, handler: cmd_ext_doctor);

fn cmd_ext_diag(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	let mut out = Vec::new();

	out.push("--- Extension Registry ---".to_string());
	out.push(format!("Commands: {}", crate::ext::all_commands().count()));
	out.push(format!("Actions:  {}", crate::ext::all_actions().count()));
	out.push(format!("Motions:  {}", crate::ext::all_motions().count()));
	out.push(format!(
		"Objects:  {}",
		crate::ext::all_text_objects().count()
	));

	let diag = diagnostics();
	if !diag.collisions.is_empty() {
		out.push(format!("\nTotal Collisions: {}", diag.collisions.len()));
		for c in &diag.collisions {
			out.push(format!(
				"  {} collision on '{}': {} (from {}) shadowed by {} (from {}) (priority {} vs {})",
				c.kind,
				c.key,
				c.shadowed_id,
				c.shadowed_source,
				c.winner_id,
				c.winner_source,
				c.shadowed_priority,
				c.winner_priority
			));
		}
	} else {
		out.push("\nNo collisions detected.".to_string());
	}

	ctx.message(&out.join("\n"));
	Ok(CommandOutcome::Ok)
}

fn cmd_ext_doctor(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	let diag = diagnostics();
	if diag.collisions.is_empty() {
		ctx.message("All good! No collisions found.");
		return Ok(CommandOutcome::Ok);
	}

	let mut out = Vec::new();
	out.push(format!("Found {} collisions:", diag.collisions.len()));

	for c in &diag.collisions {
		out.push(format!("\n[Collision] {} '{}'", c.kind, c.key));
		out.push(format!(
			"   Winner:   {} (from {}, priority {})",
			c.winner_id, c.winner_source, c.winner_priority
		));
		out.push(format!(
			"   Shadowed: {} (from {}, priority {})",
			c.shadowed_id, c.shadowed_source, c.shadowed_priority
		));

		out.push("  Fix plan:".to_string());
		if c.winner_priority == c.shadowed_priority {
			out.push(format!(
				"    - Increase priority of '{}' to at least {}",
				c.shadowed_id,
				c.winner_priority + 1
			));
			out.push(format!(
				"    - OR rename '{}' or '{}'",
				c.winner_id, c.shadowed_id
			));
		} else {
			out.push(format!(
				"    - Note: '{}' wins because its priority ({}) is higher than '{}' ({})",
				c.winner_id, c.winner_priority, c.shadowed_id, c.shadowed_priority
			));
			out.push(format!(
				"    - To reverse this, set priority of '{}' to at least {}",
				c.shadowed_id,
				c.winner_priority + 1
			));
		}
	}

	ctx.message(&out.join("\n"));
	Ok(CommandOutcome::Ok)
}
