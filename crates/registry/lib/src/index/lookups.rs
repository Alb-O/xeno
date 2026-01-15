//! Public lookup functions for registry queries.

use xeno_registry_core::ActionId;

use super::get_registry;
use crate::actions::{ActionDef, ActionKey};
use crate::commands::CommandDef;
use crate::motions::{MotionDef, MotionKey};
use crate::textobj::TextObjectDef;

/// Finds a command definition by name or alias.
pub fn find_command(name: &str) -> Option<&'static CommandDef> {
	let reg = get_registry();
	reg.commands
		.by_name
		.get(name)
		.or_else(|| reg.commands.by_alias.get(name))
		.copied()
}

/// Finds an action definition by name or alias.
pub fn find_action(name: &str) -> Option<&'static ActionDef> {
	let reg = get_registry();
	reg.actions
		.base
		.by_name
		.get(name)
		.or_else(|| reg.actions.base.by_alias.get(name))
		.copied()
}

/// Look up an action by typed ActionId.
///
/// This is the preferred method for dispatch after keybinding resolution.
pub fn find_action_by_id(id: ActionId) -> Option<&'static ActionDef> {
	if !id.is_valid() {
		return None;
	}
	let reg = get_registry();
	reg.actions.by_action_id.get(id.0 as usize).copied()
}

/// Resolve an action name to its ActionId.
///
/// Used during keybinding resolution to convert string-based bindings to typed IDs.
pub fn resolve_action_id(name: &str) -> Option<ActionId> {
	let reg = get_registry();
	reg.actions
		.name_to_id
		.get(name)
		.or_else(|| reg.actions.alias_to_id.get(name))
		.copied()
}

/// Resolve an action key to its ActionId.
pub fn resolve_action_key(key: ActionKey) -> Option<ActionId> {
	resolve_action_id(key.name())
}

/// Finds a motion definition by name or alias.
pub fn find_motion(name: &str) -> Option<MotionKey> {
	let reg = get_registry();
	reg.motions
		.by_name
		.get(name)
		.or_else(|| reg.motions.by_alias.get(name))
		.copied()
		.map(MotionKey::new)
}

/// Finds a text object definition by its trigger character.
pub fn find_text_object_by_trigger(trigger: char) -> Option<&'static TextObjectDef> {
	let reg = get_registry();
	reg.text_objects.by_trigger.get(&trigger).copied()
}

/// Returns an iterator over all command definitions, sorted by name.
pub fn all_commands() -> impl Iterator<Item = &'static CommandDef> {
	let mut v: Vec<_> = get_registry().commands.by_name.values().copied().collect();
	v.sort_by_key(|c| c.name);
	v.into_iter()
}

/// Returns an iterator over all action definitions, sorted by name.
pub fn all_actions() -> impl Iterator<Item = &'static ActionDef> {
	let mut v: Vec<_> = get_registry()
		.actions
		.base
		.by_name
		.values()
		.copied()
		.collect();
	v.sort_by_key(|a| a.name);
	v.into_iter()
}

/// Returns an iterator over all motion definitions, sorted by name.
pub fn all_motions() -> impl Iterator<Item = &'static MotionDef> {
	let mut v: Vec<_> = get_registry().motions.by_name.values().copied().collect();
	v.sort_by_key(|m| m.name);
	v.into_iter()
}

/// Returns an iterator over all text object definitions, sorted by name.
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
