use std::sync::Arc;

use super::compiler::KeymapCompiler;
use super::snapshot::KeymapSnapshot;
use super::sources::collect_keymap_spec;
use crate::actions::ActionEntry;
use crate::config::UnresolvedKeys;
use crate::core::ActionId;
use crate::core::index::Snapshot;
use crate::keymaps::KeymapPreset;

impl KeymapSnapshot {
	/// Build a snapshot from the default preset and no overrides.
	pub fn build(actions: &Snapshot<ActionEntry, ActionId>) -> Self {
		Self::build_with_overrides(actions, None)
	}

	/// Build a snapshot from the default preset with optional overrides.
	pub fn build_with_overrides(actions: &Snapshot<ActionEntry, ActionId>, overrides: Option<&UnresolvedKeys>) -> Self {
		let preset = crate::keymaps::preset(crate::keymaps::DEFAULT_PRESET);
		Self::build_with_preset(actions, preset.as_deref(), overrides)
	}

	/// Build a snapshot from explicit preset and optional overrides.
	pub fn build_with_preset(actions: &Snapshot<ActionEntry, ActionId>, preset: Option<&KeymapPreset>, overrides: Option<&UnresolvedKeys>) -> Self {
		let spec = collect_keymap_spec(actions, preset, overrides);
		let compiled = KeymapCompiler::new(actions, spec).compile();
		compiled.into_snapshot()
	}
}

/// Immutable keymap snapshot cache keyed by catalog version.
pub struct KeymapSnapshotCache {
	catalog_version: u64,
	snapshot: Arc<KeymapSnapshot>,
}

impl KeymapSnapshotCache {
	pub fn new(catalog_version: u64, snap: Arc<Snapshot<ActionEntry, ActionId>>) -> Self {
		let snapshot = Arc::new(KeymapSnapshot::build(&snap));
		Self { catalog_version, snapshot }
	}

	pub fn snapshot(&self) -> Arc<KeymapSnapshot> {
		Arc::clone(&self.snapshot)
	}

	pub fn catalog_version(&self) -> u64 {
		self.catalog_version
	}
}

/// Backward-compatible alias while call sites migrate from old naming.
pub type KeymapRegistry = KeymapSnapshotCache;

/// Returns the keymap snapshot for the immutable actions registry catalog.
pub fn get_keymap_snapshot() -> Arc<KeymapSnapshot> {
	let db = crate::db::get_catalog();
	db.keymap.snapshot()
}
