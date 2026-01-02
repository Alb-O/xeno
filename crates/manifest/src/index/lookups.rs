//! Public lookup functions for registry queries.

use evildoer_registry::motions::MotionDef;

use super::get_registry;
use crate::text_objects::TextObjectDef;
use crate::{ActionDef, ActionId, CommandDef};

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

pub fn find_motion(name: &str) -> Option<&'static MotionDef> {
	let reg = get_registry();
	reg.motions
		.by_name
		.get(name)
		.or_else(|| reg.motions.by_alias.get(name))
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
