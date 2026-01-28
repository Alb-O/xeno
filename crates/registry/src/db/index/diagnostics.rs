//! Registry diagnostics for collision detection.
//!
//! Provides reports on registry collisions where multiple items share the
//! same key, allowing users to identify and resolve registration conflicts.
//!
//! Collisions are aggregated from the core registries which enforce invariants:
//! - ID uniqueness (always fatal, never appears in collision reports)
//! - No name/alias shadowing IDs (always fatal)
//! - Name/alias conflicts are reported here for diagnostics

use crate::core::{Collision as CoreCollision, KeyKind, RegistryEntry};
use crate::db::index::collision::CollisionKind;
use crate::db::{ACTIONS, COMMANDS, MOTIONS, TEXT_OBJECTS};

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

/// Converts a core collision to a collision report by looking up definitions.
fn core_collision_to_report<T: RegistryEntry + 'static>(
	collision: &CoreCollision,
	lookup: impl Fn(&str) -> Option<&'static T>,
) -> Option<CollisionReport> {
	let winner = lookup(collision.winner_id)?;
	let shadowed = lookup(collision.existing_id)?;

	Some(CollisionReport {
		kind: match collision.kind {
			KeyKind::Id => CollisionKind::Id,
			KeyKind::Name => CollisionKind::Name,
			KeyKind::Alias => CollisionKind::Alias,
		},
		key: collision.key.to_string(),
		winner_id: collision.winner_id,
		winner_source: winner.source().to_string(),
		winner_priority: winner.priority(),
		shadowed_id: collision.existing_id,
		shadowed_source: shadowed.source().to_string(),
		shadowed_priority: shadowed.priority(),
	})
}

/// Generates a diagnostic report aggregating collisions from all core registries.
pub fn diagnostics() -> DiagnosticReport {
	let mut reports = Vec::new();

	macro_rules! collect {
		($registry:expr) => {
			for collision in $registry.collisions() {
				if let Some(report) =
					core_collision_to_report(&collision, |id| $registry.get_by_id(id))
				{
					reports.push(report);
				}
			}
		};
	}

	collect!(ACTIONS);
	collect!(COMMANDS);
	collect!(MOTIONS);
	collect!(TEXT_OBJECTS);

	DiagnosticReport {
		collisions: reports,
	}
}
