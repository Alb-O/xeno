//! Registry indexing and lookup for editor extensions.
//!
//! This module provides compile-time distributed slice indexing for actions, commands,
//! motions, text objects, and file types.

mod builders;
mod collision;
mod diagnostics;
mod lookups;
mod types;

pub use collision::{Collision, CollisionKind};
pub use diagnostics::{CollisionReport, DiagnosticReport, diagnostics};
pub use lookups::{
	all_actions, all_commands, all_motions, all_text_objects, find_action, find_action_by_id,
	find_command, find_motion, find_text_object_by_name, find_text_object_by_trigger,
	resolve_action_id,
};
pub use types::{ActionRegistryIndex, ExtensionRegistry, RegistryIndex};

use std::sync::OnceLock;

static REGISTRY: OnceLock<ExtensionRegistry> = OnceLock::new();

pub fn get_registry() -> &'static ExtensionRegistry {
	REGISTRY.get_or_init(builders::build_registry)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Capability;

	#[test]
	fn test_no_unimplemented_capabilities() {
		let reg = get_registry();
		let unimplemented = [Capability::Jump, Capability::Macro, Capability::Transform];

		for cmd in reg.commands.by_id.values() {
			for cap in cmd.required_caps {
				assert!(
					!unimplemented.contains(cap),
					"Command '{}' requires unimplemented capability: {:?}",
					cmd.id,
					cap
				);
			}
		}

		for action in reg.actions.base.by_id.values() {
			for cap in action.required_caps {
				assert!(
					!unimplemented.contains(cap),
					"Action '{}' requires unimplemented capability: {:?}",
					action.id,
					cap
				);
			}
		}
	}

	#[test]
	fn test_action_id_resolution() {
		use crate::ActionId;

		let move_left_id = resolve_action_id("move_left");
		assert!(
			move_left_id.is_some(),
			"move_left should resolve to ActionId"
		);
		let id = move_left_id.unwrap();
		assert!(id.is_valid(), "ActionId should be valid");

		let action = find_action_by_id(id);
		assert!(action.is_some(), "should find action by ActionId");
		assert_eq!(action.unwrap().name, "move_left");

		let invalid = find_action_by_id(crate::ActionId::INVALID);
		assert!(invalid.is_none(), "INVALID ActionId should return None");

		let by_name = find_action("move_left").unwrap();
		let by_id = find_action_by_id(id).unwrap();
		assert_eq!(
			by_name.name, by_id.name,
			"find_action and find_action_by_id should return the same action"
		);
	}
}
