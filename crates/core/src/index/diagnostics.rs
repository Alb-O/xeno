//! Registry diagnostics for collision detection.
//!
//! Provides reports on registry collisions where multiple items share the
//! same key, allowing users to identify and resolve registration conflicts.

use xeno_registry::RegistryMetadata;

use crate::index::ExtensionRegistry;
use crate::index::collision::CollisionKind;

/// Report containing all detected registry collisions.
pub struct DiagnosticReport {
	/// List of all collision reports across registries.
	pub collisions: Vec<CollisionReport>,
}

/// Details about a single registry collision.
pub struct CollisionReport {
	/// Type of collision (by ID, by name, or by key binding).
	pub kind: CollisionKind,
	/// The key that caused the collision.
	pub key: String,
	/// ID of the item that won the collision (higher priority).
	pub winner_id: &'static str,
	/// Source of the winning item.
	pub winner_source: String,
	/// Priority of the winning item.
	pub winner_priority: i16,
	/// ID of the item that was shadowed.
	pub shadowed_id: &'static str,
	/// Source of the shadowed item.
	pub shadowed_source: String,
	/// Priority of the shadowed item.
	pub shadowed_priority: i16,
}

/// Generates diagnostics for a specific registry.
pub(crate) fn diagnostics_internal(reg: &ExtensionRegistry) -> DiagnosticReport {
	let mut reports = Vec::new();

	macro_rules! collect {
		($index:expr) => {
			for c in &$index.collisions {
				reports.push(CollisionReport {
					kind: c.kind,
					key: c.key.clone(),
					winner_id: c.winner.id(),
					winner_source: c.winner.source().to_string(),
					winner_priority: c.winner.priority(),
					shadowed_id: c.shadowed.id(),
					shadowed_source: c.shadowed.source().to_string(),
					shadowed_priority: c.shadowed.priority(),
				});
			}
		};
	}

	collect!(reg.commands);
	collect!(reg.actions.base);
	collect!(reg.motions);
	collect!(reg.text_objects);

	DiagnosticReport {
		collisions: reports,
	}
}

/// Generates a diagnostic report for the global extension registry.
pub fn diagnostics() -> DiagnosticReport {
	diagnostics_internal(super::get_registry())
}
