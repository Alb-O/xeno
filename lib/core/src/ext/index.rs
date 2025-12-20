use std::collections::HashMap;
use std::sync::OnceLock;

use crate::ext::{
	ACTIONS, ActionDef, COMMANDS, CommandDef, MOTIONS, MotionDef, TEXT_OBJECTS, TextObjectDef,
};

pub struct RegistryIndex<T: 'static> {
	pub by_id: HashMap<&'static str, &'static T>,
	pub by_name: HashMap<&'static str, &'static T>,
	pub by_alias: HashMap<&'static str, &'static T>,
	pub collisions: Vec<Collision>,
}

#[derive(Debug, Clone)]
pub struct Collision {
	pub key: String,
	pub first_id: &'static str,
	pub second_id: &'static str,
	pub source: &'static str, // "name" or "alias" or "trigger"
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
			collisions: Vec::new(),
		}
	}
}

pub struct ExtensionRegistry {
	pub commands: RegistryIndex<CommandDef>,
	pub actions: RegistryIndex<ActionDef>,
	pub motions: RegistryIndex<MotionDef>,
	pub text_objects: RegistryIndex<TextObjectDef>,
}

static REGISTRY: OnceLock<ExtensionRegistry> = OnceLock::new();

pub fn get_registry() -> &'static ExtensionRegistry {
	REGISTRY.get_or_init(build_registry)
}

fn build_registry() -> ExtensionRegistry {
	let mut commands: RegistryIndex<CommandDef> = RegistryIndex::new();
	let mut sorted_commands: Vec<_> = COMMANDS.iter().collect();
	// Deterministic ordering: higher priority first, then name
	sorted_commands.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.name.cmp(b.name)));

	for cmd in sorted_commands {
		if let Some(existing) = commands.by_name.get(cmd.name) {
			commands.collisions.push(Collision {
				key: cmd.name.to_string(),
				first_id: existing.id,
				second_id: cmd.id,
				source: "name",
			});
		} else {
			commands.by_name.insert(cmd.name, cmd);
		}
		commands.by_id.insert(cmd.id, cmd);

		for alias in cmd.aliases {
			if let Some(existing) = commands.by_alias.get(alias) {
				commands.collisions.push(Collision {
					key: alias.to_string(),
					first_id: existing.id,
					second_id: cmd.id,
					source: "alias",
				});
			} else {
				commands.by_alias.insert(alias, cmd);
			}
		}
	}

	let mut actions: RegistryIndex<ActionDef> = RegistryIndex::new();
	let mut sorted_actions: Vec<_> = ACTIONS.iter().collect();
	sorted_actions.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.name.cmp(b.name)));

	for action in sorted_actions {
		if let Some(existing) = actions.by_name.get(action.name) {
			actions.collisions.push(Collision {
				key: action.name.to_string(),
				first_id: existing.id,
				second_id: action.id,
				source: "name",
			});
		} else {
			actions.by_name.insert(action.name, action);
		}
		actions.by_id.insert(action.id, action);
	}

	let mut motions: RegistryIndex<MotionDef> = RegistryIndex::new();
	for motion in MOTIONS {
		motions.by_name.insert(motion.name, motion);
		motions.by_id.insert(motion.id, motion);
	}

	let mut text_objects: RegistryIndex<TextObjectDef> = RegistryIndex::new();
	for obj in TEXT_OBJECTS {
		text_objects.by_name.insert(obj.name, obj);
		text_objects.by_id.insert(obj.id, obj);
		// Note: we could also index by trigger here
	}

	ExtensionRegistry {
		commands,
		actions,
		motions,
		text_objects,
	}
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
	reg.actions.by_name.get(name).copied()
}

pub fn validate_all_registries() -> Vec<Collision> {
	let reg = get_registry();
	let mut all_collisions = Vec::new();
	all_collisions.extend(reg.commands.collisions.clone());
	all_collisions.extend(reg.actions.collisions.clone());
	all_collisions.extend(reg.motions.collisions.clone());
	all_collisions.extend(reg.text_objects.collisions.clone());

	if cfg!(debug_assertions) && !all_collisions.is_empty() {
		for c in &all_collisions {
			log::error!(
				"Extension collision on {} '{}': {} shadowed by {}",
				c.source,
				c.key,
				c.second_id,
				c.first_id
			);
		}
	}

	all_collisions
}
