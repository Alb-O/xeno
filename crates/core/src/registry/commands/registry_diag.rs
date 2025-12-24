use futures::future::LocalBoxFuture;

use crate::command;
use crate::registry::index::diagnostics;
use crate::registry::{CommandContext, CommandError, CommandOutcome};

command!(registry_diag, { aliases: &["registry.diag"], description: "Show registry system diagnostics" }, handler: cmd_registry_diag);
command!(registry_doctor, { aliases: &["registry.doctor"], description: "Check for registry collisions and suggest fixes" }, handler: cmd_registry_doctor);

fn cmd_registry_diag<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let mut out = Vec::new();

		out.push("--- Registry Inventory ---".to_string());
		out.push(format!(
			"Commands: {}",
			crate::registry::all_commands().count()
		));
		out.push(format!(
			"Actions:  {}",
			crate::registry::all_actions().count()
		));
		out.push(format!(
			"Motions:  {}",
			crate::registry::all_motions().count()
		));
		out.push(format!(
			"Objects:  {}",
			crate::registry::all_text_objects().count()
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
	})
}

fn cmd_registry_doctor<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
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
	})
}
