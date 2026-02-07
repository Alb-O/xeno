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

	pub fn by_kdl_key(&self, key: &str) -> Option<OptionsRef> {
		self.inner.get(key)
	}

	pub fn items(&self) -> Vec<OptionsRef> {
		self.inner.all()
	}

	pub fn all(&self) -> Vec<OptionsRef> {
		self.inner.all()
	}

	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}
}
