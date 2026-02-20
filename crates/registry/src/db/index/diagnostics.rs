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
	let diagnostics = crate::db::get_catalog().diagnostics();
	DiagnosticReport {
		collisions: diagnostics.collisions,
	}
}
