use crate::core::index::{RegistryIndex, RegistryRef};
use crate::core::{DenseId, RuntimeRegistry, TextObjectId};
use crate::domains::shared::precedence::promote_if_winner;

/// Guard object that keeps a text-object snapshot alive while providing access to a definition.
pub type TextObjectRef = RegistryRef<crate::textobj::TextObjectEntry, TextObjectId>;

use crate::textobj::TextObjectEntry;

pub struct TextObjectRegistry {
	pub(super) inner: RuntimeRegistry<TextObjectEntry, TextObjectId>,
}

impl TextObjectRegistry {
	pub fn new(builtins: RegistryIndex<TextObjectEntry, TextObjectId>) -> Self {
		Self {
			inner: RuntimeRegistry::new("text_objects", builtins),
		}
	}

	pub fn get(&self, key: &str) -> Option<TextObjectRef> {
		self.inner.get(key)
	}

	pub fn by_trigger(&self, trigger: char) -> Option<TextObjectRef> {
		let snap = self.inner.snapshot();
		let mut winner: Option<(TextObjectId, crate::core::Party)> = None;

		for (idx, entry) in snap.table.iter().enumerate() {
			if entry.trigger != trigger && !entry.alt_triggers.contains(&trigger) {
				continue;
			}

			let id = TextObjectId::from_u32(idx as u32);
			let party = snap.parties[idx];
			promote_if_winner(&mut winner, id, party);
		}

		winner.map(|(id, _)| RegistryRef { snap, id })
	}

	pub fn all(&self) -> Vec<TextObjectRef> {
		self.inner.snapshot_guard().iter_refs().collect()
	}

	pub fn snapshot_guard(&self) -> crate::core::index::SnapshotGuard<TextObjectEntry, TextObjectId> {
		self.inner.snapshot_guard()
	}

	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	pub fn collisions(&self) -> &[crate::core::Collision] {
		self.inner.collisions()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::core::index::RegistryBuilder;
	use crate::core::{RegistryMetaStatic, RegistrySource};
	use crate::textobj::{TextObjectDef, TextObjectEntry, TextObjectInput};

	fn test_inner(_text: ropey::RopeSlice, _pos: usize) -> Option<xeno_primitives::Range> {
		None
	}

	fn test_around(_text: ropey::RopeSlice, _pos: usize) -> Option<xeno_primitives::Range> {
		None
	}

	static BUILTIN_TEXT_OBJECT: TextObjectDef = TextObjectDef {
		meta: RegistryMetaStatic {
			id: "registry::textobj::builtin",
			name: "builtin",
			keys: &[],
			description: "builtin text object",
			priority: 0,
			source: RegistrySource::Builtin,
			mutates_buffer: false,
		},
		trigger: 'x',
		alt_triggers: &[],
		inner: test_inner,
		around: test_around,
	};

	static RUNTIME_TEXT_OBJECT: TextObjectDef = TextObjectDef {
		meta: RegistryMetaStatic {
			id: "registry::textobj::runtime",
			name: "runtime",
			keys: &[],
			description: "runtime text object",
			priority: 0,
			source: RegistrySource::Runtime,
			mutates_buffer: false,
		},
		trigger: 'x',
		alt_triggers: &[],
		inner: test_inner,
		around: test_around,
	};

	#[test]
	fn by_trigger_prefers_runtime_source_on_tie() {
		let mut builder: RegistryBuilder<TextObjectInput, TextObjectEntry, TextObjectId> = RegistryBuilder::new("textobj-test");
		builder.push(std::sync::Arc::new(TextObjectInput::Static(BUILTIN_TEXT_OBJECT)));
		builder.push(std::sync::Arc::new(TextObjectInput::Static(RUNTIME_TEXT_OBJECT)));

		let registry = TextObjectRegistry::new(builder.build());

		let resolved = registry.by_trigger('x').expect("trigger should resolve");
		assert_eq!(resolved.id_str(), RUNTIME_TEXT_OBJECT.meta.id);
	}
}
