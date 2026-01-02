//! Registry index types and core data structures.

use std::collections::HashMap;

use evildoer_registry::motions::MotionDef;

use super::collision::Collision;
use crate::text_objects::TextObjectDef;
use crate::{ActionDef, ActionId, CommandDef};

/// Generic registry index with collision tracking.
pub struct RegistryIndex<T: 'static> {
	pub by_id: HashMap<&'static str, &'static T>,
	pub by_name: HashMap<&'static str, &'static T>,
	pub by_alias: HashMap<&'static str, &'static T>,
	pub by_trigger: HashMap<char, &'static T>,
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
	pub commands: RegistryIndex<CommandDef>,
	pub actions: ActionRegistryIndex,
	pub motions: RegistryIndex<MotionDef>,
	pub text_objects: RegistryIndex<TextObjectDef>,
}
