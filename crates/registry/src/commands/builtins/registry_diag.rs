//! Registry diagnostics commands for detecting and reporting collisions.
//!
//! These commands help identify when multiple registry items (commands, motions,
//! text objects) share the same ID, name, alias, or trigger, and report which
//! item wins based on priority.

use std::collections::HashMap;

use futures::future::LocalBoxFuture;

use crate::commands::{
	COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome, command,
};
use crate::notifications::keys;
use crate::{motions, textobj};

command!(
	registry_diag,
	{ aliases: &["registry.diag"], description: "Show registry system diagnostics" },
	handler: cmd_registry_diag
);
command!(
	registry_doctor,
	{
		aliases: &["registry.doctor"],
		description: "Check for registry collisions and suggest fixes"
	},
	handler: cmd_registry_doctor
);

/// Type of collision detected between registry items.
enum CollisionKind {
	/// Multiple items share the same unique identifier.
	Id,
	/// Multiple items share the same display name.
	Name,
	/// An alias conflicts with another item's name or alias.
	Alias,
	/// Multiple text objects share the same trigger character.
	Trigger,
}

impl std::fmt::Display for CollisionKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Id => write!(f, "ID"),
			Self::Name => write!(f, "name"),
			Self::Alias => write!(f, "alias"),
			Self::Trigger => write!(f, "trigger"),
		}
	}
}

/// Details about a collision between two registry items.
struct CollisionReport {
	/// The type of collision (ID, name, alias, or trigger).
	kind: CollisionKind,
	/// The conflicting key value (the shared ID, name, alias, or trigger).
	key: String,
	/// ID of the item that wins due to higher priority.
	winner_id: &'static str,
	/// Source location or extension name of the winner.
	winner_source: String,
	/// Priority value of the winning item.
	winner_priority: i16,
	/// ID of the item that is shadowed.
	shadowed_id: &'static str,
	/// Source location or extension name of the shadowed item.
	shadowed_source: String,
	/// Priority value of the shadowed item.
	shadowed_priority: i16,
}

/// Aggregated diagnostic report for all registries.
struct DiagnosticReport {
	/// All detected collisions across commands, motions, and text objects.
	collisions: Vec<CollisionReport>,
}

/// Collects diagnostics from all registries.
fn diagnostics() -> DiagnosticReport {
	let mut collisions = Vec::new();
	collect_command_collisions(&mut collisions);
	collect_motion_collisions(&mut collisions);
	collect_text_object_collisions(&mut collisions);
	DiagnosticReport { collisions }
}

/// Checks for a collision when registering a command and records it if found.
fn register_command_collision(
	kind: CollisionKind,
	key: &'static str,
	current: &'static CommandDef,
	map: &mut HashMap<&'static str, &'static CommandDef>,
	collisions: &mut Vec<CollisionReport>,
) {
	if let Some(existing) = map.get(key).copied() {
		let (winner, shadowed) = if current.priority() > existing.priority() {
			map.insert(key, current);
			(current, existing)
		} else {
			(existing, current)
		};
		collisions.push(CollisionReport {
			kind,
			key: key.to_string(),
			winner_id: winner.id(),
			winner_source: winner.source().to_string(),
			winner_priority: winner.priority(),
			shadowed_id: shadowed.id(),
			shadowed_source: shadowed.source().to_string(),
			shadowed_priority: shadowed.priority(),
		});
	} else {
		map.insert(key, current);
	}
}

/// Iterates all registered commands and detects ID, name, and alias collisions.
fn collect_command_collisions(collisions: &mut Vec<CollisionReport>) {
	let mut by_id = HashMap::new();
	let mut by_name = HashMap::new();
	let mut by_alias = HashMap::new();

	for cmd in COMMANDS.iter() {
		register_command_collision(CollisionKind::Id, cmd.id(), cmd, &mut by_id, collisions);
		register_command_collision(
			CollisionKind::Name,
			cmd.name(),
			cmd,
			&mut by_name,
			collisions,
		);
		for &alias in cmd.aliases() {
			register_command_collision(CollisionKind::Alias, alias, cmd, &mut by_alias, collisions);
		}
	}
}

/// Checks for a collision when registering a motion and records it if found.
fn register_motion_collision(
	kind: CollisionKind,
	key: &'static str,
	current: &'static motions::MotionDef,
	map: &mut HashMap<&'static str, &'static motions::MotionDef>,
	collisions: &mut Vec<CollisionReport>,
) {
	if let Some(existing) = map.get(key).copied() {
		let (winner, shadowed) = if current.priority() > existing.priority() {
			map.insert(key, current);
			(current, existing)
		} else {
			(existing, current)
		};
		collisions.push(CollisionReport {
			kind,
			key: key.to_string(),
			winner_id: winner.id(),
			winner_source: winner.source().to_string(),
			winner_priority: winner.priority(),
			shadowed_id: shadowed.id(),
			shadowed_source: shadowed.source().to_string(),
			shadowed_priority: shadowed.priority(),
		});
	} else {
		map.insert(key, current);
	}
}

/// Iterates all registered motions and detects ID, name, and alias collisions.
fn collect_motion_collisions(collisions: &mut Vec<CollisionReport>) {
	let mut by_id = HashMap::new();
	let mut by_name = HashMap::new();
	let mut by_alias = HashMap::new();

	for motion in motions::all() {
		register_motion_collision(
			CollisionKind::Id,
			motion.id(),
			motion,
			&mut by_id,
			collisions,
		);
		register_motion_collision(
			CollisionKind::Name,
			motion.name(),
			motion,
			&mut by_name,
			collisions,
		);
		for &alias in motion.aliases() {
			register_motion_collision(
				CollisionKind::Alias,
				alias,
				motion,
				&mut by_alias,
				collisions,
			);
		}
	}
}

/// Checks for a collision when registering a text object and records it if found.
fn register_text_object_collision(
	kind: CollisionKind,
	key: String,
	current: &'static textobj::TextObjectDef,
	map: &mut HashMap<String, &'static textobj::TextObjectDef>,
	collisions: &mut Vec<CollisionReport>,
) {
	if let Some(existing) = map.get(&key).copied() {
		let (winner, shadowed) = if current.priority() > existing.priority() {
			map.insert(key.clone(), current);
			(current, existing)
		} else {
			(existing, current)
		};
		collisions.push(CollisionReport {
			kind,
			key,
			winner_id: winner.id(),
			winner_source: winner.source().to_string(),
			winner_priority: winner.priority(),
			shadowed_id: shadowed.id(),
			shadowed_source: shadowed.source().to_string(),
			shadowed_priority: shadowed.priority(),
		});
	} else {
		map.insert(key, current);
	}
}

/// Iterates all text objects and detects ID, name, alias, and trigger collisions.
fn collect_text_object_collisions(collisions: &mut Vec<CollisionReport>) {
	let mut by_id = HashMap::new();
	let mut by_name = HashMap::new();
	let mut by_alias = HashMap::new();
	let mut by_trigger = HashMap::new();

	for obj in textobj::all() {
		register_text_object_collision(
			CollisionKind::Id,
			obj.id().to_string(),
			obj,
			&mut by_id,
			collisions,
		);
		register_text_object_collision(
			CollisionKind::Name,
			obj.name().to_string(),
			obj,
			&mut by_name,
			collisions,
		);
		for &alias in obj.aliases() {
			register_text_object_collision(
				CollisionKind::Alias,
				alias.to_string(),
				obj,
				&mut by_alias,
				collisions,
			);
		}
		register_text_object_collision(
			CollisionKind::Trigger,
			obj.trigger.to_string(),
			obj,
			&mut by_trigger,
			collisions,
		);
		for trigger in obj.alt_triggers {
			register_text_object_collision(
				CollisionKind::Trigger,
				trigger.to_string(),
				obj,
				&mut by_trigger,
				collisions,
			);
		}
	}
}

/// Handler for the `:registry.diag` command showing registry inventory and collisions.
fn cmd_registry_diag<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let mut out = Vec::new();

		out.push("--- Registry Inventory ---".to_string());
		out.push(format!("Commands: {}", COMMANDS.len()));
		out.push("Actions:  0".to_string());
		out.push(format!("Motions:  {}", motions::all().count()));
		out.push(format!("Objects:  {}", textobj::all().count()));

		let diag = diagnostics();
		let has_collisions = !diag.collisions.is_empty();
		if has_collisions {
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

		if has_collisions {
			ctx.emit(keys::diagnostic_warning(out.join("\n")));
		} else {
			ctx.emit(keys::diagnostic_output(out.join("\n")));
		}
		Ok(CommandOutcome::Ok)
	})
}

/// Handler for the `:registry.doctor` command with detailed collision analysis and fix suggestions.
fn cmd_registry_doctor<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let diag = diagnostics();
		if diag.collisions.is_empty() {
			ctx.emit(keys::NO_COLLISIONS);
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

		ctx.emit(keys::diagnostic_warning(out.join("\n")));
		Ok(CommandOutcome::Ok)
	})
}
