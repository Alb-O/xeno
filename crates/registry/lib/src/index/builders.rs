//! Registry builder logic - constructs indices from registered definitions.
//!
//! Builds secondary indices for ExtensionRegistry (ActionId mapping, trigger lookup).
//! Collision tracking and invariant enforcement is handled by core registries.

use std::collections::HashMap;

use xeno_registry_core::ActionId;

use super::types::{ActionRegistryIndex, ExtensionRegistry, RegistryIndex};
use crate::actions::ActionDef;
use crate::builder::RegistryBuilder;
use crate::builtins;
use crate::commands::CommandDef;
use crate::motions::MotionDef;
use crate::textobj::TextObjectDef;

/// Builds the complete extension registry from registered definitions.
///
/// Core registries have already validated invariants during initialization.
/// This builds secondary indices for ActionId dispatch and trigger-based lookup.
pub(super) fn build_registry() -> ExtensionRegistry {
	let mut builder = RegistryBuilder::new();
	if let Err(err) = builtins::register_all(&mut builder) {
		panic!("Registry registration failed: {err}");
	}
	builder
		.build()
		.unwrap_or_else(|err| panic!("Registry build failed: {err}"))
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
///
/// Invariants and collisions are handled by the core COMMANDS registry.
/// This builds secondary indices for ExtensionRegistry.
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
		index.by_id.insert(cmd.meta.id, cmd);
		index.by_name.entry(cmd.meta.name).or_insert(cmd);
		for &alias in cmd.meta.aliases {
			index.by_alias.entry(alias).or_insert(cmd);
		}
	}

	index
}

/// Builds the action registry index with ActionId mappings.
///
/// Invariants and collisions are handled by the core ACTIONS registry.
/// This builds secondary indices for ActionId dispatch.
fn build_action_index_from_defs(actions: &[&'static ActionDef]) -> ActionRegistryIndex {
	let mut base = RegistryIndex::new();
	let mut by_action_id: Vec<&'static ActionDef> = Vec::new();
	let mut name_to_id: HashMap<&'static str, ActionId> = HashMap::new();
	let mut alias_to_id: HashMap<&'static str, ActionId> = HashMap::new();

	let mut sorted: Vec<_> = actions.to_vec();
	sorted.sort_by(|a, b| b.priority().cmp(&a.priority()).then(a.id().cmp(b.id())));

	for action in sorted {
		base.by_id.insert(action.id(), action);

		if base.by_name.get(action.name()).is_none() {
			let action_id = ActionId(by_action_id.len() as u32);
			by_action_id.push(action);
			name_to_id.insert(action.name(), action_id);
			base.by_name.insert(action.name(), action);
		}

		for alias in action.aliases() {
			if base.by_name.get(alias).is_none() && base.by_alias.get(alias).is_none() {
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
///
/// Invariants and collisions are handled by the core MOTIONS registry.
/// This builds secondary indices for ExtensionRegistry.
fn build_motion_index_from_defs(motions: &[&'static MotionDef]) -> RegistryIndex<MotionDef> {
	let mut index = RegistryIndex::new();
	let mut sorted: Vec<_> = motions.to_vec();
	sorted.sort_by(|a, b| b.priority().cmp(&a.priority()).then(a.id().cmp(b.id())));

	for motion in sorted {
		index.by_id.insert(motion.id(), motion);
		index.by_name.entry(motion.name()).or_insert(motion);
		for alias in motion.aliases() {
			index.by_alias.entry(alias).or_insert(motion);
		}
	}

	index
}

/// Builds the text object registry index from text object definitions.
///
/// Invariants and collisions are handled by core TEXT_OBJECTS.
/// Trigger lookup uses first-wins based on priority sort order.
fn build_text_object_index_from_defs(
	text_objects: &[&'static TextObjectDef],
) -> RegistryIndex<TextObjectDef> {
	let mut index = RegistryIndex::new();
	let mut sorted: Vec<_> = text_objects.to_vec();
	sorted.sort_by(|a, b| b.priority().cmp(&a.priority()).then(a.id().cmp(b.id())));

	for obj in sorted {
		index.by_id.insert(obj.id(), obj);
		index.by_name.entry(obj.name()).or_insert(obj);
		for alias in obj.aliases() {
			index.by_alias.entry(alias).or_insert(obj);
		}
		index.by_trigger.entry(obj.trigger).or_insert(obj);
		for &trigger in obj.alt_triggers {
			index.by_trigger.entry(trigger).or_insert(obj);
		}
	}

	index
}
