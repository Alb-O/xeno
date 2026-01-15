//! Registry index types and core data structures.

use std::collections::HashMap;

use xeno_registry_core::ActionId;

use super::collision::Collision;
use crate::actions::ActionDef;
use crate::commands::CommandDef;
use crate::motions::MotionDef;
use crate::textobj::TextObjectDef;

/// Generic registry index with collision tracking.
pub struct RegistryIndex<T: 'static> {
	/// Lookup by unique identifier.
	pub by_id: HashMap<&'static str, &'static T>,
	/// Lookup by display name.
	pub by_name: HashMap<&'static str, &'static T>,
	/// Lookup by alternative name/alias.
	pub by_alias: HashMap<&'static str, &'static T>,
	/// Lookup by trigger character (for motions/text objects).
	pub by_trigger: HashMap<char, &'static T>,
	/// Collisions detected during index construction.
	pub collisions: Vec<Collision<T>>,
}

/// Index for actions with typed ActionId support.
pub struct ActionRegistryIndex {
	/// Standard registry index for string-based lookups.
	pub base: RegistryIndex<ActionDef>,
	/// Map from ActionId to ActionDef for fast dispatch.
	pub by_action_id: Vec<&'static ActionDef>,
	/// Map from action name to ActionId for resolving keybindings.
	pub name_to_id: HashMap<&'static str, ActionId>,
	/// Map from alias to ActionId.
	pub alias_to_id: HashMap<&'static str, ActionId>,
}

impl<T: 'static> Default for RegistryIndex<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: 'static> RegistryIndex<T> {
	/// Creates an empty registry index.
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

/// Central registry for all editor extensions.
pub struct ExtensionRegistry {
	/// Index for editor commands.
	pub commands: RegistryIndex<CommandDef>,
	/// Index for actions with fast ActionId dispatch.
	pub actions: ActionRegistryIndex,
	/// Index for cursor motions.
	pub motions: RegistryIndex<MotionDef>,
	/// Index for text objects.
	pub text_objects: RegistryIndex<TextObjectDef>,
}
