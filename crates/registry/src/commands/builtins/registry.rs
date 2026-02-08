use std::collections::HashMap;

use xeno_primitives::BoxFutureLocal;

use crate::commands::{COMMANDS, CommandContext, CommandError, CommandOutcome, RegistryEntry};
use crate::notifications::keys;
use crate::{command_handler, motions, textobj};

command_handler!(registry_diag, handler: cmd_registry_diag);

command_handler!(registry_doctor, handler: cmd_registry_doctor);

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
	winner_id: String,
	winner_source: String,
	winner_priority: i16,
	shadowed_id: String,
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

struct EntryMeta {
	id: String,
	source: String,
	priority: i16,
}

fn register_collision(
	kind: CollisionKind,
	key: String,
	current: EntryMeta,
	map: &mut HashMap<String, EntryMeta>,
	collisions: &mut Vec<CollisionReport>,
) {
	if let Some(existing) = map.get(&key) {
		let current_won = current.priority > existing.priority;
		let (winner, shadowed) = if current_won {
			(current, existing.clone())
		} else {
			(existing.clone(), current)
		};

		if current_won {
			map.insert(key.clone(), winner.clone());
		}

		collisions.push(CollisionReport {
			kind,
			key,
			winner_id: winner.id,
			winner_source: winner.source,
			winner_priority: winner.priority,
			shadowed_id: shadowed.id,
			shadowed_source: shadowed.source,
			shadowed_priority: shadowed.priority,
		});
	} else {
		map.insert(key, current);
	}
}

fn collect_command_collisions(collisions: &mut Vec<CollisionReport>) {
	let mut by_id = HashMap::new();
	let mut by_name = HashMap::new();
	let mut by_alias = HashMap::new();

	for cmd in COMMANDS.all() {
		let meta = EntryMeta {
			id: cmd.id_str().to_string(),
			source: cmd.source().to_string(),
			priority: cmd.priority(),
		};

		register_collision(
			CollisionKind::Id,
			cmd.id_str().to_string(),
			meta.clone(),
			&mut by_id,
			collisions,
		);
		register_collision(
			CollisionKind::Name,
			cmd.name_str().to_string(),
			meta.clone(),
			&mut by_name,
			collisions,
		);
		for alias in cmd.keys_resolved() {
			register_collision(
				CollisionKind::Alias,
				alias.to_string(),
				meta.clone(),
				&mut by_alias,
				collisions,
			);
		}
	}
}

fn collect_motion_collisions(collisions: &mut Vec<CollisionReport>) {
	let mut by_id = HashMap::new();
	let mut by_name = HashMap::new();
	let mut by_alias = HashMap::new();

	for motion in motions::all() {
		let meta = EntryMeta {
			id: motion.id_str().to_string(),
			source: motion.source().to_string(),
			priority: motion.priority(),
		};

		register_collision(
			CollisionKind::Id,
			motion.id_str().to_string(),
			meta.clone(),
			&mut by_id,
			collisions,
		);
		register_collision(
			CollisionKind::Name,
			motion.name_str().to_string(),
			meta.clone(),
			&mut by_name,
			collisions,
		);
		for alias in motion.keys_resolved() {
			register_collision(
				CollisionKind::Alias,
				alias.to_string(),
				meta.clone(),
				&mut by_alias,
				collisions,
			);
		}
	}
}

fn collect_text_object_collisions(collisions: &mut Vec<CollisionReport>) {
	let mut by_trigger = HashMap::new();

	for obj in textobj::all() {
		let meta = EntryMeta {
			id: obj.id_str().to_string(),
			source: obj.source().to_string(),
			priority: obj.priority(),
		};

		register_collision(
			CollisionKind::Trigger,
			obj.trigger.to_string(),
			meta.clone(),
			&mut by_trigger,
			collisions,
		);
		for &trigger in &*obj.alt_triggers {
			register_collision(
				CollisionKind::Trigger,
				trigger.to_string(),
				meta.clone(),
				&mut by_trigger,
				collisions,
			);
		}
	}
}

impl Clone for EntryMeta {
	fn clone(&self) -> Self {
		Self {
			id: self.id.clone(),
			source: self.source.clone(),
			priority: self.priority,
		}
	}
}

fn cmd_registry_diag<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
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
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
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
