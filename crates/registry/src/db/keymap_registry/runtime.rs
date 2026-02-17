use std::sync::Arc;

use arc_swap::ArcSwap;

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

/// Reactive keymap snapshot cache tied to the actions snapshot lifecycle.
pub struct KeymapSnapshotCache {
	cache: ArcSwap<KeymapCache>,
}

struct KeymapCache {
	snap: Arc<Snapshot<ActionEntry, ActionId>>,
	snapshot: Arc<KeymapSnapshot>,
}

impl KeymapSnapshotCache {
	pub fn new(snap: Arc<Snapshot<ActionEntry, ActionId>>) -> Self {
		let snapshot = Arc::new(KeymapSnapshot::build(&snap));
		Self {
			cache: ArcSwap::from_pointee(KeymapCache { snap, snapshot }),
		}
	}

	pub fn for_snapshot(&self, snap: Arc<Snapshot<ActionEntry, ActionId>>) -> Arc<KeymapSnapshot> {
		let current = self.cache.load();
		if Arc::ptr_eq(&current.snap, &snap) {
			return Arc::clone(&current.snapshot);
		}

		let snapshot = Arc::new(KeymapSnapshot::build(&snap));
		self.cache.store(Arc::new(KeymapCache {
			snap: Arc::clone(&snap),
			snapshot: Arc::clone(&snapshot),
		}));
		snapshot
	}
}

/// Backward-compatible alias while call sites migrate from old naming.
pub type KeymapRegistry = KeymapSnapshotCache;

/// Returns the keymap snapshot for the current actions registry snapshot.
pub fn get_keymap_snapshot() -> Arc<KeymapSnapshot> {
	let db = crate::db::get_db();
	db.keymap.for_snapshot(crate::db::ACTIONS.snapshot())
}
