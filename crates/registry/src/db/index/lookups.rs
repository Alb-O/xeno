//! Public lookup functions for registry queries.

use crate::actions::{ActionDef, ActionKey};
use crate::commands::CommandDef;
use crate::core::ActionId;
use crate::db::{
	ACTIONS, COMMANDS, MOTIONS, TEXT_OBJECTS, resolve_action_id_from_static,
	resolve_action_id_typed,
};
use crate::motions::{MotionDef, MotionKey};
use crate::textobj::TextObjectRef;

/// Finds a command definition by ID, name, or alias.
pub fn find_command(key: &str) -> Option<crate::core::RegistryRef<CommandDef>> {
	COMMANDS.get(key)
}

/// Finds an action definition by ID, name, or alias.
pub fn find_action(key: &str) -> Option<crate::core::RegistryRef<ActionDef>> {
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
	MOTIONS.get(key).map(MotionKey::new_ref)
}

/// Finds a text object definition by its trigger character.
pub fn find_text_object_by_trigger(trigger: char) -> Option<TextObjectRef> {
	TEXT_OBJECTS.by_trigger(trigger)
}

/// Returns all command definitions, sorted by name.
pub fn all_commands() -> Vec<crate::core::RegistryRef<CommandDef>> {
	let mut v = COMMANDS.all();
	v.sort_by_key(|c| c.name().to_string());
	v
}

/// Returns all action definitions, sorted by name.
pub fn all_actions() -> Vec<crate::core::RegistryRef<ActionDef>> {
	let mut v = ACTIONS.all();
	v.sort_by_key(|a| a.name().to_string());
	v
}

/// Returns all motion definitions, sorted by name.
pub fn all_motions() -> Vec<crate::core::RegistryRef<MotionDef>> {
	let mut v = MOTIONS.all();
	v.sort_by_key(|m| m.name().to_string());
	v
}

/// Returns all text object definitions, sorted by name.
pub fn all_text_objects() -> Vec<TextObjectRef> {
	let mut v = TEXT_OBJECTS.all();
	v.sort_by_key(|o| o.name().to_string());
	v
}
