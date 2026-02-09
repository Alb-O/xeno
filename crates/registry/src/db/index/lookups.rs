//! Public lookup functions for registry queries.

use crate::actions::{ActionEntry, ActionKey};
use crate::commands::CommandEntry;
use crate::core::{ActionId, CommandId, LookupKey, MotionId, RegistryRef};
use crate::db::{ACTIONS, COMMANDS, MOTIONS, TEXT_OBJECTS, resolve_action_id_typed};
use crate::motions::MotionEntry;
use crate::textobj::TextObjectRef;

/// Finds a command definition by ID, name, or key.
pub fn find_command(key: &str) -> Option<RegistryRef<CommandEntry, CommandId>> {
	COMMANDS.get(key)
}

/// Finds an action definition by ID, name, or key.
pub fn find_action(key: &str) -> Option<RegistryRef<ActionEntry, ActionId>> {
	ACTIONS.get(key)
}

/// Look up an action by typed ActionId.
pub fn find_action_by_id(id: ActionId) -> Option<std::sync::Arc<ActionEntry>> {
	resolve_action_id_typed(id)
}

/// Resolve an action name to its ActionId.
pub fn resolve_action_id(name: &str) -> Option<ActionId> {
	ACTIONS
		.get(name)
		.map(|a: crate::actions::ActionRef| a.dense_id())
}

/// Resolve an action key to its ActionId.
pub fn resolve_action_key(key: ActionKey) -> Option<ActionId> {
	match &key {
		LookupKey::Static(canonical_id) => ACTIONS.get(canonical_id).map(|r| r.dense_id()),
		LookupKey::Ref(r) => Some(r.dense_id()),
	}
}

/// Finds a motion definition by ID, name, or key.
pub fn find_motion(key: &str) -> Option<RegistryRef<MotionEntry, MotionId>> {
	MOTIONS.get(key)
}

/// Finds a text object definition by its trigger character.
pub fn find_text_object_by_trigger(trigger: char) -> Option<TextObjectRef> {
	TEXT_OBJECTS.by_trigger(trigger)
}

/// Returns all command definitions, sorted by name.
pub fn all_commands() -> Vec<RegistryRef<CommandEntry, CommandId>> {
	COMMANDS.snapshot_guard().iter_refs().collect()
}

/// Returns all action definitions, sorted by name.
pub fn all_actions() -> Vec<RegistryRef<ActionEntry, ActionId>> {
	ACTIONS.snapshot_guard().iter_refs().collect()
}

/// Returns all motion definitions, sorted by name.
pub fn all_motions() -> Vec<RegistryRef<MotionEntry, MotionId>> {
	MOTIONS.snapshot_guard().iter_refs().collect()
}

/// Returns all text object definitions, sorted by name.
pub fn all_text_objects() -> Vec<TextObjectRef> {
	TEXT_OBJECTS.snapshot_guard().iter_refs().collect()
}
