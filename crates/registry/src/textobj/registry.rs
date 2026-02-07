use std::sync::Arc;

use rustc_hash::FxHashMap as HashMap;

use crate::core::TextObjectId;
use crate::core::index::{RegistryIndex, RegistryRef};

/// Guard object that keeps a text-object snapshot alive while providing access to a definition.
pub type TextObjectRef = RegistryRef<crate::textobj::TextObjectEntry, TextObjectId>;

use crate::core::RuntimeRegistry;
use crate::textobj::TextObjectEntry;

pub struct TextObjectRegistry {
	pub(super) inner: RuntimeRegistry<TextObjectEntry, TextObjectId>,
	pub(super) by_trigger: Arc<HashMap<char, TextObjectId>>,
}

impl TextObjectRegistry {
	pub fn new(builtins: RegistryIndex<TextObjectEntry, TextObjectId>) -> Self {
		let mut trigger_map = HashMap::default();
		// Build trigger map from builtins
		for (idx, entry) in builtins.items().iter().enumerate() {
			let id = crate::core::DenseId::from_u32(idx as u32);
			trigger_map.insert(entry.trigger, id);
			for &alt in &*entry.alt_triggers {
				trigger_map.insert(alt, id);
			}
		}

		Self {
			inner: RuntimeRegistry::new("text_objects", builtins),
			by_trigger: Arc::new(trigger_map),
		}
	}

	pub fn get(&self, key: &str) -> Option<TextObjectRef> {
		self.inner.get(key)
	}

	pub fn by_trigger(&self, trigger: char) -> Option<TextObjectRef> {
		let id = *self.by_trigger.get(&trigger)?;
		// We assume IDs are stable and the map is in sync with the inner registry.
		// For static builtins this is true.
		// For runtime extensions, we would need to update by_trigger.
		// This wrapper implementation is incomplete for runtime registration of new triggers.
		// But for reading, it works.
		self.inner.get_by_id(id) // Requires get_by_id on RuntimeRegistry (added in previous step)
	}

	pub fn all(&self) -> Vec<TextObjectRef> {
		self.inner.all()
	}

	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}
}
