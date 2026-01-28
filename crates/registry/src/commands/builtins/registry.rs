use std::collections::HashMap;

use futures::future::LocalBoxFuture;

use crate::commands::{COMMANDS, CommandContext, CommandError, CommandOutcome};
use crate::notifications::keys;
use crate::{command, motions, textobj};

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

enum CollisionKind {
	Id,
	Name,
	Alias,
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

struct CollisionReport {
	kind: CollisionKind,
	key: String,
	winner_id: &'static str,
	winner_source: String,
	winner_priority: i16,
	shadowed_id: &'static str,
	shadowed_source: String,
	shadowed_priority: i16,
}

struct DiagnosticReport {
	collisions: Vec<CollisionReport>,
}

fn diagnostics() -> DiagnosticReport {
	let mut collisions = Vec::new();
	collect_command_collisions(&mut collisions);
	collect_motion_collisions(&mut collisions);
	collect_text_object_collisions(&mut collisions);
	DiagnosticReport { collisions }
}

fn register_command_collision(
	kind: CollisionKind,
	key: &'static str,
	current: &'static crate::commands::CommandDef,
	map: &mut HashMap<&'static str, &'static crate::commands::CommandDef>,
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

fn register_text_object_collision(
	kind: CollisionKind,
	key: char,
	current: &'static textobj::TextObjectDef,
	map: &mut HashMap<char, &'static textobj::TextObjectDef>,
	collisions: &mut Vec<CollisionReport>,
) {
	if let Some(existing) = map.get(&key).copied() {
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

fn collect_text_object_collisions(collisions: &mut Vec<CollisionReport>) {
	let mut by_trigger = HashMap::new();

	for obj in textobj::all() {
		register_text_object_collision(
			CollisionKind::Trigger,
			obj.trigger,
			obj,
			&mut by_trigger,
			collisions,
		);
		for &trigger in obj.alt_triggers {
			register_text_object_collision(
				CollisionKind::Trigger,
				trigger,
				obj,
				&mut by_trigger,
				collisions,
			);
		}
	}
}

fn cmd_registry_diag<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let report = diagnostics();
		if report.collisions.is_empty() {
			ctx.emit(keys::NO_COLLISIONS);
			return Ok(CommandOutcome::Ok);
		}

		let mut out = Vec::new();
		out.push("Registry collisions detected:".to_string());
		for c in &report.collisions {
			out.push(format!(
				"[{}] {} '{}' -> winner: {} ({} @ {}), shadowed: {} ({} @ {})",
				c.kind,
				match c.kind {
					CollisionKind::Id => "ID",
					CollisionKind::Name => "name",
					CollisionKind::Alias => "alias",
					CollisionKind::Trigger => "trigger",
				},
				c.key,
				c.winner_id,
				c.winner_source,
				c.winner_priority,
				c.shadowed_id,
				c.shadowed_source,
				c.shadowed_priority
			));
		}
		ctx.emit(keys::diagnostic_output(out.join("\n")));
		Ok(CommandOutcome::Ok)
	})
}

fn cmd_registry_doctor<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let report = diagnostics();
		if report.collisions.is_empty() {
			ctx.emit(keys::NO_COLLISIONS);
			return Ok(CommandOutcome::Ok);
		}

		let mut out = Vec::new();
		out.push("Registry Doctor Report:".to_string());
		out.push("Potential fixes:".to_string());
		for c in &report.collisions {
			out.push(format!(
				"- [{}] '{}' shadowed by '{}' (priority {}). Consider increasing priority or renaming.",
				c.kind, c.shadowed_id, c.winner_id, c.winner_priority
			));
		}
		ctx.emit(keys::diagnostic_warning(out.join("\n")));
		Ok(CommandOutcome::Ok)
	})
}

pub const DEFS: &[&crate::commands::CommandDef] = &[&CMD_registry_diag, &CMD_registry_doctor];
