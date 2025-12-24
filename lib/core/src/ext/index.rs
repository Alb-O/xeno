use std::collections::HashMap;
use std::sync::OnceLock;

use crate::ext::{
	ACTIONS, ActionDef, COMMANDS, CommandDef, ExtensionMetadata, FILE_TYPES, FileTypeDef, MOTIONS,
	MotionDef, TEXT_OBJECTS, TextObjectDef,
};

pub struct RegistryIndex<T: 'static> {
	pub by_id: HashMap<&'static str, &'static T>,
	pub by_name: HashMap<&'static str, &'static T>,
	pub by_alias: HashMap<&'static str, &'static T>,
	pub by_trigger: HashMap<char, &'static T>,
	pub collisions: Vec<Collision<T>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionKind {
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

pub struct Collision<T: 'static> {
	pub kind: CollisionKind,
	pub key: String,
	pub winner: &'static T,
	pub shadowed: &'static T,
}

impl<T: 'static> Default for RegistryIndex<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: 'static> RegistryIndex<T> {
	pub fn new() -> Self {
		Self {
			by_id: HashMap::new(),
			by_name: HashMap::new(),
			by_alias: HashMap::new(),
			by_trigger: HashMap::new(),
			collisions: Vec::new(),
		}
	}
}

pub struct ExtensionRegistry {
	pub commands: RegistryIndex<CommandDef>,
	pub actions: RegistryIndex<ActionDef>,
	pub motions: RegistryIndex<MotionDef>,
	pub text_objects: RegistryIndex<TextObjectDef>,
	pub file_types: RegistryIndex<FileTypeDef>,
}

static REGISTRY: OnceLock<ExtensionRegistry> = OnceLock::new();

pub fn get_registry() -> &'static ExtensionRegistry {
	REGISTRY.get_or_init(build_registry)
}

fn build_registry() -> ExtensionRegistry {
	let mut commands: RegistryIndex<CommandDef> = RegistryIndex::new();
	let mut sorted_commands: Vec<_> = COMMANDS.iter().collect();
	sorted_commands.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));

	for cmd in sorted_commands {
		if let Some(existing) = commands.by_id.get(cmd.id) {
			commands.collisions.push(Collision {
				kind: CollisionKind::Id,
				key: cmd.id.to_string(),
				winner: existing,
				shadowed: cmd,
			});
		} else {
			commands.by_id.insert(cmd.id, cmd);
		}

		if let Some(existing) = commands.by_name.get(cmd.name) {
			commands.collisions.push(Collision {
				kind: CollisionKind::Name,
				key: cmd.name.to_string(),
				winner: existing,
				shadowed: cmd,
			});
		} else {
			commands.by_name.insert(cmd.name, cmd);
		}

		for alias in cmd.aliases {
			if let Some(existing) = commands.by_name.get(alias) {
				commands.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: cmd,
				});
			} else if let Some(existing) = commands.by_alias.get(alias) {
				commands.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: cmd,
				});
			} else {
				commands.by_alias.insert(alias, cmd);
			}
		}
	}

	let mut actions: RegistryIndex<ActionDef> = RegistryIndex::new();
	let mut sorted_actions: Vec<_> = ACTIONS.iter().collect();
	sorted_actions.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));

	for action in sorted_actions {
		if let Some(existing) = actions.by_id.get(action.id) {
			actions.collisions.push(Collision {
				kind: CollisionKind::Id,
				key: action.id.to_string(),
				winner: existing,
				shadowed: action,
			});
		} else {
			actions.by_id.insert(action.id, action);
		}

		if let Some(existing) = actions.by_name.get(action.name) {
			actions.collisions.push(Collision {
				kind: CollisionKind::Name,
				key: action.name.to_string(),
				winner: existing,
				shadowed: action,
			});
		} else {
			actions.by_name.insert(action.name, action);
		}

		for alias in action.aliases {
			if let Some(existing) = actions.by_name.get(alias) {
				actions.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: action,
				});
			} else if let Some(existing) = actions.by_alias.get(alias) {
				actions.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: action,
				});
			} else {
				actions.by_alias.insert(alias, action);
			}
		}
	}

	let mut motions: RegistryIndex<MotionDef> = RegistryIndex::new();
	let mut sorted_motions: Vec<_> = MOTIONS.iter().collect();
	sorted_motions.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));

	for motion in sorted_motions {
		if let Some(existing) = motions.by_id.get(motion.id) {
			motions.collisions.push(Collision {
				kind: CollisionKind::Id,
				key: motion.id.to_string(),
				winner: existing,
				shadowed: motion,
			});
		} else {
			motions.by_id.insert(motion.id, motion);
		}

		if let Some(existing) = motions.by_name.get(motion.name) {
			motions.collisions.push(Collision {
				kind: CollisionKind::Name,
				key: motion.name.to_string(),
				winner: existing,
				shadowed: motion,
			});
		} else {
			motions.by_name.insert(motion.name, motion);
		}

		for alias in motion.aliases {
			if let Some(existing) = motions.by_name.get(alias) {
				motions.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: motion,
				});
			} else if let Some(existing) = motions.by_alias.get(alias) {
				motions.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: motion,
				});
			} else {
				motions.by_alias.insert(alias, motion);
			}
		}
	}

	let mut text_objects: RegistryIndex<TextObjectDef> = RegistryIndex::new();
	let mut sorted_objects: Vec<_> = TEXT_OBJECTS.iter().collect();
	sorted_objects.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));

	for obj in sorted_objects {
		if let Some(existing) = text_objects.by_id.get(obj.id) {
			text_objects.collisions.push(Collision {
				kind: CollisionKind::Id,
				key: obj.id.to_string(),
				winner: existing,
				shadowed: obj,
			});
		} else {
			text_objects.by_id.insert(obj.id, obj);
		}

		if let Some(existing) = text_objects.by_name.get(obj.name) {
			text_objects.collisions.push(Collision {
				kind: CollisionKind::Name,
				key: obj.name.to_string(),
				winner: existing,
				shadowed: obj,
			});
		} else {
			text_objects.by_name.insert(obj.name, obj);
		}

		for alias in obj.aliases {
			if let Some(existing) = text_objects.by_name.get(alias) {
				text_objects.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: obj,
				});
			} else if let Some(existing) = text_objects.by_alias.get(alias) {
				text_objects.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: obj,
				});
			} else {
				text_objects.by_alias.insert(alias, obj);
			}
		}

		if let Some(existing) = text_objects.by_trigger.get(&obj.trigger) {
			text_objects.collisions.push(Collision {
				kind: CollisionKind::Trigger,
				key: obj.trigger.to_string(),
				winner: existing,
				shadowed: obj,
			});
		} else {
			text_objects.by_trigger.insert(obj.trigger, obj);
		}

		// Index by alternative triggers
		for trigger in obj.alt_triggers {
			if let Some(existing) = text_objects.by_trigger.get(trigger) {
				text_objects.collisions.push(Collision {
					kind: CollisionKind::Trigger,
					key: trigger.to_string(),
					winner: existing,
					shadowed: obj,
				});
			} else {
				text_objects.by_trigger.insert(*trigger, obj);
			}
		}
	}

	let mut file_types: RegistryIndex<FileTypeDef> = RegistryIndex::new();
	let mut sorted_file_types: Vec<_> = FILE_TYPES.iter().collect();
	sorted_file_types.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));

	for ft in sorted_file_types {
		if let Some(existing) = file_types.by_id.get(ft.id) {
			file_types.collisions.push(Collision {
				kind: CollisionKind::Id,
				key: ft.id.to_string(),
				winner: existing,
				shadowed: ft,
			});
		} else {
			file_types.by_id.insert(ft.id, ft);
		}

		// Index by name (used for :set ft=<name>)
		if let Some(existing) = file_types.by_name.get(ft.name) {
			file_types.collisions.push(Collision {
				kind: CollisionKind::Name,
				key: ft.name.to_string(),
				winner: existing,
				shadowed: ft,
			});
		} else {
			file_types.by_name.insert(ft.name, ft);
		}

		for ext in ft.extensions {
			if let Some(existing) = file_types.by_alias.get(ext) {
				file_types.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: ext.to_string(),
					winner: existing,
					shadowed: ft,
				});
			} else {
				file_types.by_alias.insert(ext, ft);
			}
		}

		for fname in ft.filenames {
			if let Some(existing) = file_types.by_alias.get(fname) {
				file_types.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: fname.to_string(),
					winner: existing,
					shadowed: ft,
				});
			} else {
				file_types.by_alias.insert(fname, ft);
			}
		}
	}

	let registry = ExtensionRegistry {
		commands,
		actions,
		motions,
		text_objects,
		file_types,
	};

	let diag = diagnostics_internal(&registry);
	if !diag.collisions.is_empty() {
		let fatal_collisions: Vec<_> = diag
			.collisions
			.iter()
			.filter(|c| c.winner_priority == c.shadowed_priority)
			.collect();

		if !fatal_collisions.is_empty() && cfg!(debug_assertions) {
			let mut msg =
				String::from("Unresolved extension collisions (equal priority) in debug build:\n");
			for c in &fatal_collisions {
				msg.push_str(&format!(
					"  {} collision on '{}': {} (from {}) and {} (from {}) both have priority {}\n",
					c.kind,
					c.key,
					c.shadowed_id,
					c.shadowed_source,
					c.winner_id,
					c.winner_source,
					c.winner_priority
				));
			}
			msg.push_str("Please resolve these collisions by renaming or adjusting priorities.");
			panic!("{}", msg);
		}

		if cfg!(debug_assertions) {
			for c in &diag.collisions {
				if c.winner_priority != c.shadowed_priority {
					log::debug!(
						"Extension shadowing: {} '{}' from {} shadowed by {} due to priority ({} vs {})",
						c.kind,
						c.key,
						c.shadowed_source,
						c.winner_id,
						c.winner_priority,
						c.shadowed_priority
					);
				}
			}
		} else {
			log::warn!("Extension collisions detected. Use :ext doctor to resolve.");
		}
	}

	registry
}

pub fn find_command(name: &str) -> Option<&'static CommandDef> {
	let reg = get_registry();
	reg.commands
		.by_name
		.get(name)
		.or_else(|| reg.commands.by_alias.get(name))
		.copied()
}

pub fn find_action(name: &str) -> Option<&'static ActionDef> {
	let reg = get_registry();
	reg.actions
		.by_name
		.get(name)
		.or_else(|| reg.actions.by_alias.get(name))
		.copied()
}

pub fn find_motion(name: &str) -> Option<&'static MotionDef> {
	let reg = get_registry();
	reg.motions
		.by_name
		.get(name)
		.or_else(|| reg.motions.by_alias.get(name))
		.copied()
}

pub fn find_text_object_by_name(name: &str) -> Option<&'static TextObjectDef> {
	let reg = get_registry();
	reg.text_objects
		.by_name
		.get(name)
		.or_else(|| reg.text_objects.by_alias.get(name))
		.copied()
}

pub fn find_text_object_by_trigger(trigger: char) -> Option<&'static TextObjectDef> {
	let reg = get_registry();
	reg.text_objects.by_trigger.get(&trigger).copied()
}

pub fn all_commands() -> impl Iterator<Item = &'static CommandDef> {
	let mut v: Vec<_> = get_registry().commands.by_name.values().copied().collect();
	v.sort_by_key(|c| c.name);
	v.into_iter()
}

pub fn all_actions() -> impl Iterator<Item = &'static ActionDef> {
	let mut v: Vec<_> = get_registry().actions.by_name.values().copied().collect();
	v.sort_by_key(|a| a.name);
	v.into_iter()
}

pub fn all_motions() -> impl Iterator<Item = &'static MotionDef> {
	let mut v: Vec<_> = get_registry().motions.by_name.values().copied().collect();
	v.sort_by_key(|m| m.name);
	v.into_iter()
}

pub fn all_text_objects() -> impl Iterator<Item = &'static TextObjectDef> {
	let mut v: Vec<_> = get_registry()
		.text_objects
		.by_name
		.values()
		.copied()
		.collect();
	v.sort_by_key(|o| o.name);
	v.into_iter()
}

pub struct DiagnosticReport {
	pub collisions: Vec<CollisionReport>,
}

pub struct CollisionReport {
	pub kind: CollisionKind,
	pub key: String,
	pub winner_id: &'static str,
	pub winner_source: String,
	pub winner_priority: i16,
	pub shadowed_id: &'static str,
	pub shadowed_source: String,
	pub shadowed_priority: i16,
}

fn diagnostics_internal(reg: &ExtensionRegistry) -> DiagnosticReport {
	let mut reports = Vec::new();

	macro_rules! collect {
		($index:expr) => {
			for c in &$index.collisions {
				reports.push(CollisionReport {
					kind: c.kind,
					key: c.key.clone(),
					winner_id: c.winner.id(),
					winner_source: c.winner.source().to_string(),
					winner_priority: c.winner.priority(),
					shadowed_id: c.shadowed.id(),
					shadowed_source: c.shadowed.source().to_string(),
					shadowed_priority: c.shadowed.priority(),
				});
			}
		};
	}

	collect!(reg.commands);
	collect!(reg.actions);
	collect!(reg.motions);
	collect!(reg.text_objects);
	collect!(reg.file_types);

	DiagnosticReport {
		collisions: reports,
	}
}

pub fn diagnostics() -> DiagnosticReport {
	diagnostics_internal(get_registry())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ext::Capability;

	#[test]
	fn test_no_unimplemented_capabilities() {
		let reg = get_registry();
		let unimplemented = [Capability::Jump, Capability::Macro, Capability::Transform];

		for cmd in reg.commands.by_id.values() {
			for cap in cmd.required_caps {
				assert!(
					!unimplemented.contains(cap),
					"Command '{}' requires unimplemented capability: {:?}",
					cmd.id,
					cap
				);
			}
		}

		for action in reg.actions.by_id.values() {
			for cap in action.required_caps {
				assert!(
					!unimplemented.contains(cap),
					"Action '{}' requires unimplemented capability: {:?}",
					action.id,
					cap
				);
			}
		}
	}
}
