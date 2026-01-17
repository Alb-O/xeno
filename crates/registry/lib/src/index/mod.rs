//! Registry indexing and lookup for editor extensions.
//!
//! This module provides registry indexing for actions, commands, motions, text objects,
//! and file types.

mod builders;
mod collision;
mod diagnostics;
mod lookups;
mod types;

use std::sync::OnceLock;

pub use collision::{Collision, CollisionKind};
pub use diagnostics::{CollisionReport, DiagnosticReport, diagnostics};
pub use lookups::{
	all_actions, all_commands, all_motions, all_text_objects, find_action, find_action_by_id,
	find_command, find_motion, find_text_object_by_trigger, resolve_action_id, resolve_action_key,
};
pub use types::{ActionRegistryIndex, ExtensionRegistry, RegistryIndex};
pub(crate) use builders::build_registry_from_defs;

/// Global extension registry singleton.
static REGISTRY: OnceLock<ExtensionRegistry> = OnceLock::new();

/// Returns the global extension registry, initializing it if necessary.
pub fn get_registry() -> &'static ExtensionRegistry {
	REGISTRY.get_or_init(builders::build_registry)
}

// Integration tests that require xeno-stdlib are in tests/registry.rs
