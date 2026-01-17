//! Registry builder logic - constructs indices from registered definitions.

use std::collections::HashMap;

use tracing::debug;
use xeno_registry_core::ActionId;

use super::collision::{Collision, CollisionKind};
use super::diagnostics::diagnostics_internal;
use super::types::{ActionRegistryIndex, ExtensionRegistry, RegistryIndex};
use crate::actions::ActionDef;
use crate::builder::RegistryBuilder;
use crate::commands::CommandDef;
use crate::motions::MotionDef;
use crate::textobj::TextObjectDef;
use crate::{RegistryMetadata, builtins};

/// Builds the complete extension registry from registered definitions.
///
/// Processes all registered extensions (actions, commands, motions, etc.),
/// resolves collisions based on priority, and performs validation checks.
pub(super) fn build_registry() -> ExtensionRegistry {
	let mut builder = RegistryBuilder::new();
	if let Err(err) = builtins::register_all(&mut builder) {
		panic!("Registry registration failed: {err}");
	}
	let registry = builder
		.build()
		.unwrap_or_else(|err| panic!("Registry build failed: {err}"));

	validate_registry(&registry);

	registry
}

pub(crate) fn build_registry_from_defs(
	commands: &[&'static CommandDef],
	actions: &[&'static ActionDef],
	motions: &[&'static MotionDef],
	text_objects: &[&'static TextObjectDef],
) -> ExtensionRegistry {
	let commands = build_command_index_from_defs(commands);
	let actions = build_action_index_from_defs(actions);
	let motions = build_motion_index_from_defs(motions);
	let text_objects = build_text_object_index_from_defs(text_objects);

	ExtensionRegistry {
		commands,
		actions,
		motions,
		text_objects,
	}
}

/// Builds the command registry index from command definitions.
fn build_command_index_from_defs(commands: &[&'static CommandDef]) -> RegistryIndex<CommandDef> {
	let mut index = RegistryIndex::new();
	let mut sorted: Vec<_> = commands.to_vec();
	sorted.sort_by(|a, b| {
		b.meta
			.priority
			.cmp(&a.meta.priority)
			.then(a.meta.id.cmp(b.meta.id))
	});

	for cmd in sorted {
		register_with_id_name_aliases(
			&mut index,
			cmd,
			cmd.meta.id,
			cmd.meta.name,
			cmd.meta.aliases,
		);
	}

	index
}

/// Builds the action registry index with ActionId mappings.
fn build_action_index_from_defs(actions: &[&'static ActionDef]) -> ActionRegistryIndex {
	let mut base = RegistryIndex::new();
	let mut by_action_id: Vec<&'static ActionDef> = Vec::new();
	let mut name_to_id: HashMap<&'static str, ActionId> = HashMap::new();
	let mut alias_to_id: HashMap<&'static str, ActionId> = HashMap::new();

	let mut sorted: Vec<_> = actions.to_vec();
	sorted.sort_by(|a, b| b.priority().cmp(&a.priority()).then(a.id().cmp(b.id())));

	for action in sorted {
		if let Some(&existing) = base.by_id.get(action.id()) {
			base.collisions.push(Collision {
				kind: CollisionKind::Id,
				key: action.id().to_string(),
				winner: existing,
				shadowed: action,
			});
		} else {
			base.by_id.insert(action.id(), action);
		}

		if let Some(&existing) = base.by_name.get(action.name()) {
			base.collisions.push(Collision {
				kind: CollisionKind::Name,
				key: action.name().to_string(),
				winner: existing,
				shadowed: action,
			});
		} else {
			let action_id = ActionId(by_action_id.len() as u32);
			by_action_id.push(action);
			name_to_id.insert(action.name(), action_id);
			base.by_name.insert(action.name(), action);
		}

		for alias in action.aliases() {
			if let Some(&existing) = base.by_name.get(alias) {
				base.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: action,
				});
			} else if let Some(&existing) = base.by_alias.get(alias) {
				base.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: action,
				});
			} else {
				base.by_alias.insert(alias, action);
				if let Some(&id) = name_to_id.get(action.name()) {
					alias_to_id.insert(alias, id);
				}
			}
		}
	}

	ActionRegistryIndex {
		base,
		by_action_id,
		name_to_id,
		alias_to_id,
	}
}

/// Builds the motion registry index from motion definitions.
fn build_motion_index_from_defs(motions: &[&'static MotionDef]) -> RegistryIndex<MotionDef> {
	let mut index = RegistryIndex::new();
	let mut sorted: Vec<_> = motions.to_vec();
	sorted.sort_by(|a, b| b.priority().cmp(&a.priority()).then(a.id().cmp(b.id())));

	for motion in sorted {
		register_with_id_name_aliases(
			&mut index,
			motion,
			motion.id(),
			motion.name(),
			motion.aliases(),
		);
	}

	index
}

/// Builds the text object registry index from text object definitions.
fn build_text_object_index_from_defs(
	text_objects: &[&'static TextObjectDef],
) -> RegistryIndex<TextObjectDef> {
	let mut index = RegistryIndex::new();
	let mut sorted: Vec<_> = text_objects.to_vec();
	sorted.sort_by(|a, b| b.priority().cmp(&a.priority()).then(a.id().cmp(b.id())));

	for obj in sorted {
		register_with_id_name_aliases(&mut index, obj, obj.id(), obj.name(), obj.aliases());

		if let Some(&existing) = index.by_trigger.get(&obj.trigger) {
			index.collisions.push(Collision {
				kind: CollisionKind::Trigger,
				key: obj.trigger.to_string(),
				winner: existing,
				shadowed: obj,
			});
		} else {
			index.by_trigger.insert(obj.trigger, obj);
		}

		for trigger in obj.alt_triggers {
			if let Some(&existing) = index.by_trigger.get(trigger) {
				index.collisions.push(Collision {
					kind: CollisionKind::Trigger,
					key: trigger.to_string(),
					winner: existing,
					shadowed: obj,
				});
			} else {
				index.by_trigger.insert(*trigger, obj);
			}
		}
	}

	index
}

/// Helper to register an item with id, name, and aliases.
fn register_with_id_name_aliases<T: RegistryMetadata>(
	index: &mut RegistryIndex<T>,
	item: &'static T,
	id: &'static str,
	name: &'static str,
	aliases: &'static [&'static str],
) {
	if let Some(&existing) = index.by_id.get(id) {
		index.collisions.push(Collision {
			kind: CollisionKind::Id,
			key: id.to_string(),
			winner: existing,
			shadowed: item,
		});
	} else {
		index.by_id.insert(id, item);
	}

	if let Some(&existing) = index.by_name.get(name) {
		index.collisions.push(Collision {
			kind: CollisionKind::Name,
			key: name.to_string(),
			winner: existing,
			shadowed: item,
		});
	} else {
		index.by_name.insert(name, item);
	}

	for alias in aliases {
		register_alias(index, item, alias);
	}
}

/// Helper to register a single alias with collision checking.
fn register_alias<T: RegistryMetadata>(
	index: &mut RegistryIndex<T>,
	item: &'static T,
	alias: &'static str,
) {
	if let Some(&existing) = index.by_name.get(alias) {
		// Not a collision if the alias matches the item's own name
		if !std::ptr::eq(existing, item) {
			index.collisions.push(Collision {
				kind: CollisionKind::Alias,
				key: alias.to_string(),
				winner: existing,
				shadowed: item,
			});
		}
	} else if let Some(&existing) = index.by_alias.get(alias) {
		index.collisions.push(Collision {
			kind: CollisionKind::Alias,
			key: alias.to_string(),
			winner: existing,
			shadowed: item,
		});
	} else {
		index.by_alias.insert(alias, item);
	}
}

/// Validates the registry and reports/panics on collisions.
fn validate_registry(reg: &ExtensionRegistry) {
	let diag = diagnostics_internal(reg);
	if diag.collisions.is_empty() {
		return;
	}

	for c in &diag.collisions {
		debug!(
			kind = %c.kind,
			key = c.key,
			shadowed_source = %c.shadowed_source,
			winner_id = c.winner_id,
			winner_priority = c.winner_priority,
			shadowed_priority = c.shadowed_priority,
			"Extension shadowing"
		);
	}

	let mut msg = String::from("Registry collisions detected:\n");
	for c in &diag.collisions {
		msg.push_str(&format!(
			"  {} collision on '{}': {} (from {}) and {} (from {}) priorities {} vs {}\n",
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
	msg.push_str("Please resolve these collisions by renaming or adjusting priorities.");
	panic!("{}", msg);
}
