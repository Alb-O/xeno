//! Text object registry.

use std::sync::Arc;

use ropey::RopeSlice;
use xeno_primitives::Range;

pub mod builtins;
pub mod handler;
pub mod link;
pub mod loader;
mod macros;
pub mod registry;
pub mod spec;

pub use builtins::register_builtins;
pub use handler::{TextObjectHandlerReg, TextObjectHandlerStatic};
pub use registry::{TextObjectRef, TextObjectRegistry};

use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

use crate::core::index::{BuildCtx, BuildEntry, RegistryMetaRef, StrListRef};
pub use crate::core::{
	Capability, CapabilitySet, DuplicatePolicy, FrozenInterner, RegistryBuilder, RegistryEntry,
	RegistryIndex, RegistryMeta, RegistryMetaStatic, RegistryMetadata, RegistryRef, RegistrySource,
	Symbol, SymbolList, TextObjectId,
};
// Re-export macros
pub use crate::text_object_handler;

pub type TextObjectHandler = fn(RopeSlice, usize) -> Option<Range>;

/// Definition of a text object (static input).
#[derive(Clone, Copy)]
pub struct TextObjectDef {
	pub meta: RegistryMetaStatic,
	pub trigger: char,
	pub alt_triggers: &'static [char],
	pub inner: TextObjectHandler,
	pub around: TextObjectHandler,
}

impl TextObjectDef {
	#[doc(hidden)]
	#[allow(clippy::too_many_arguments, reason = "macro-generated constructor")]
	pub const fn new(
		meta: RegistryMetaStatic,
		trigger: char,
		alt_triggers: &'static [char],
		inner: TextObjectHandler,
		around: TextObjectHandler,
	) -> Self {
		Self {
			meta,
			trigger,
			alt_triggers,
			inner,
			around,
		}
	}
}

/// Symbolized text object entry.
pub struct TextObjectEntry {
	pub meta: RegistryMeta,
	pub trigger: char,
	pub alt_triggers: Arc<[char]>,
	pub inner: TextObjectHandler,
	pub around: TextObjectHandler,
}

crate::impl_registry_entry!(TextObjectEntry);

impl BuildEntry<TextObjectEntry> for TextObjectDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			keys: StrListRef::Static(self.meta.keys),
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			required_caps: self.meta.required_caps,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name
	}

	fn collect_payload_strings<'b>(
		&'b self,
		_collector: &mut crate::core::index::StringCollector<'_, 'b>,
	) {
	}

	fn build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> TextObjectEntry {
		let meta = crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), []);

		TextObjectEntry {
			meta,
			trigger: self.trigger,
			alt_triggers: self.alt_triggers.into(),
			inner: self.inner,
			around: self.around,
		}
	}
}

/// Unified input for text object registration — either a static `TextObjectDef`
/// or a `LinkedTextObjectDef` assembled from KDL metadata + Rust handlers.
pub type TextObjectInput =
	crate::core::def_input::DefInput<TextObjectDef, crate::textobj::link::LinkedTextObjectDef>;

#[cfg(feature = "db")]
pub use crate::db::TEXT_OBJECTS;

#[cfg(feature = "db")]
pub fn find_by_trigger(trigger: char) -> Option<TextObjectRef> {
	TEXT_OBJECTS.by_trigger(trigger)
}

#[cfg(feature = "db")]
pub fn find(name: &str) -> Option<TextObjectRef> {
	TEXT_OBJECTS.get(name)
}

#[cfg(feature = "db")]
pub fn all() -> Vec<TextObjectRef> {
	TEXT_OBJECTS.snapshot_guard().iter_refs().collect()
}
