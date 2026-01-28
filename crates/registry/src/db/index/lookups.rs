//! Public lookup functions for registry queries.

use crate::actions::{ActionDef, ActionKey};
use crate::commands::CommandDef;
use crate::core::ActionId;
use crate::db::{
	ACTIONS, COMMANDS, MOTIONS, TEXT_OBJECTS, resolve_action_id_from_static,
	resolve_action_id_typed,
};
use crate::motions::{MotionDef, MotionKey};
use crate::textobj::TextObjectDef;

/// Finds a command definition by ID, name, or alias.
pub fn find_command(key: &str) -> Option<&'static CommandDef> {
	COMMANDS.get(key)
}

/// Finds an action definition by ID, name, or alias.
pub fn find_action(key: &str) -> Option<&'static ActionDef> {
	ACTIONS.get(key)
}

/// Look up an action by typed ActionId.
pub fn find_action_by_id(id: ActionId) -> Option<&'static ActionDef> {
	resolve_action_id_typed(id)
}

/// Resolve an action name to its ActionId.
pub fn resolve_action_id(name: &str) -> Option<ActionId> {
	ACTIONS
		.get(name)
		.map(|a| resolve_action_id_from_static(a.id()))
}

/// Resolve an action key to its ActionId.
pub fn resolve_action_key(key: ActionKey) -> Option<ActionId> {
	resolve_action_id(key.name())
}

/// Finds a motion definition by ID, name, or alias.
pub fn find_motion(key: &str) -> Option<MotionKey> {
	MOTIONS.get(key).map(MotionKey::new)
}

/// Finds a text object definition by its trigger character.
pub fn find_text_object_by_trigger(trigger: char) -> Option<&'static TextObjectDef> {
	TEXT_OBJECTS.by_trigger(trigger)
}

/// Returns an iterator over all command definitions, sorted by name.
pub fn all_commands() -> impl Iterator<Item = &'static CommandDef> {
	let mut v: Vec<_> = COMMANDS.all();
	v.sort_by_key(|c| c.name());
	v.into_iter()
}

/// Returns an iterator over all action definitions, sorted by name.
pub fn all_actions() -> impl Iterator<Item = &'static ActionDef> {
	let mut v: Vec<_> = ACTIONS.all();
	v.sort_by_key(|a| a.name());
	v.into_iter()
}

/// Returns an iterator over all motion definitions, sorted by name.
pub fn all_motions() -> impl Iterator<Item = &'static MotionDef> {
	let mut v: Vec<_> = MOTIONS.all();
	v.sort_by_key(|m| m.name());
	v.into_iter()
}

/// Returns an iterator over all text object definitions, sorted by name.
pub fn all_text_objects() -> impl Iterator<Item = &'static TextObjectDef> {
	let mut v: Vec<_> = TEXT_OBJECTS.all();
	v.sort_by_key(|o| o.name());
	v.into_iter()
}
