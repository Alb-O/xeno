//! Options registry wrapper.

use crate::core::{OptionId, RegistryIndex, RegistryRef, RuntimeRegistry};
use crate::options::OptionEntry;

pub type OptionsRef = RegistryRef<OptionEntry, OptionId>;

pub struct OptionsRegistry {
	pub(super) inner: RuntimeRegistry<OptionEntry, OptionId>,
}

impl OptionsRegistry {
	pub fn new(builtins: RegistryIndex<OptionEntry, OptionId>) -> Self {
		Self {
			inner: RuntimeRegistry::new("options", builtins),
		}
	}

	pub fn get(&self, name: &str) -> Option<OptionsRef> {
		self.inner.get(name)
	}

	pub fn by_key(&self, key: &str) -> Option<OptionsRef> {
		self.inner.get(key)
	}

	/// Looks up an option by its key (either static canonical ID or resolved reference).
	pub fn get_key(&self, key: &crate::options::OptionKey) -> Option<OptionsRef> {
		self.inner.get_key(key)
	}

	pub fn items(&self) -> Vec<OptionsRef> {
		self.inner.snapshot_guard().iter_refs().collect()
	}

	pub fn all(&self) -> Vec<OptionsRef> {
		self.inner.snapshot_guard().iter_refs().collect()
	}

	pub fn snapshot_guard(&self) -> crate::core::index::SnapshotGuard<OptionEntry, OptionId> {
		self.inner.snapshot_guard()
	}

	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}
}
