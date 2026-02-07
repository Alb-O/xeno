//! Registry diagnostics for collision detection.
//!
//! Provides reports on registry collisions where multiple items share the
//! same key, allowing users to identify and resolve registration conflicts.

use crate::core::Collision;

/// Report containing all detected registry collisions.
pub struct DiagnosticReport {
	pub collisions: Vec<Collision>,
}

/// Generates a diagnostic report aggregating collisions from all core registries.
pub fn diagnostics() -> DiagnosticReport {
	let db = crate::db::get_db();
	let mut collisions = Vec::new();

	// Actions, commands, motions have RuntimeRegistry which exposes collisions
	// through the snapshot's collisions field. For now, collect from builtins.
	let snap = db.actions.snapshot();
	collisions.extend(snap.collisions.iter().cloned());
	let snap = db.commands.snapshot();
	collisions.extend(snap.collisions.iter().cloned());
	let snap = db.motions.snapshot();
	collisions.extend(snap.collisions.iter().cloned());

	DiagnosticReport { collisions }
}
