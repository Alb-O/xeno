use std::sync::Arc;

use rustc_hash::FxHashMap as HashMap;

use crate::HookEvent;
use crate::core::index::RegistryRef;
use crate::core::{DenseId, HookId, RegistryIndex, RuntimeRegistry};
use crate::hooks::{HookDef, HookEntry};

/// Guard object that keeps a hooks snapshot alive while providing access to a definition.
pub type HooksRef = RegistryRef<HookEntry, HookId>;

pub struct HooksRegistry {
	pub(super) inner: RuntimeRegistry<HookEntry, HookId>,
	pub(super) by_event: Arc<HashMap<HookEvent, Vec<HookId>>>,
}

impl HooksRegistry {
	pub fn new(builtins: RegistryIndex<HookEntry, HookId>) -> Self {
		let mut event_map: HashMap<HookEvent, Vec<HookId>> = HashMap::default();
		for (idx, entry) in builtins.items().iter().enumerate() {
			let id: HookId = DenseId::from_u32(idx as u32);
			event_map.entry(entry.event).or_default().push(id);
		}

		Self {
			inner: RuntimeRegistry::new("hooks", builtins),
			by_event: Arc::new(event_map),
		}
	}

	pub fn get(&self, key: &str) -> Option<HooksRef> {
		self.inner.get(key)
	}

	pub fn all(&self) -> Vec<HooksRef> {
		self.inner.snapshot_guard().iter_refs().collect()
	}

	pub fn snapshot_guard(&self) -> crate::core::index::SnapshotGuard<HookEntry, HookId> {
		self.inner.snapshot_guard()
	}

	pub fn for_event(&self, event: HookEvent) -> Vec<HooksRef> {
		let snap = self.inner.snapshot();
		let ids = self.by_event.get(&event).map(|v| v.as_slice()).unwrap_or(&[]);

		let mut refs = Vec::with_capacity(ids.len());
		for &id in ids {
			refs.push(RegistryRef { snap: snap.clone(), id });
		}
		refs
	}

	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	/// Runtime registration stub (not yet fully implemented).
	pub fn register(&self, _def: &'static HookDef) -> bool {
		false
	}
}
