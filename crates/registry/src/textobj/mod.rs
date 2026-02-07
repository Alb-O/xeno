//! Text object registry.

use ropey::RopeSlice;
use xeno_primitives::Range;

pub mod builtins;
mod macros;
pub mod registry;

pub use builtins::register_builtins;
pub use registry::{TextObjectRef, TextObjectRegistry};

use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

use crate::core::index::{BuildEntry, RegistryMetaRef};
pub use crate::core::{
	Capability, CapabilitySet, DuplicatePolicy, FrozenInterner, RegistryBuilder, RegistryEntry,
	RegistryIndex, RegistryMeta, RegistryMetaStatic, RegistryMetadata, RegistryRef, RegistrySource,
	Symbol, SymbolList, TextObjectId,
};
pub use crate::motions::{flags, movement};
// Re-export macros
pub use crate::text_object;
pub use crate::{bracket_pair_object, symmetric_text_object};

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

impl crate::core::RegistryEntry for TextObjectDef {
	fn meta(&self) -> &RegistryMeta {
		panic!("Called meta() on static TextObjectDef")
	}
}

/// Symbolized text object entry.
pub struct TextObjectEntry {
	pub meta: RegistryMeta,
	pub trigger: char,
	pub alt_triggers: &'static [char],
	pub inner: TextObjectHandler,
	pub around: TextObjectHandler,
}

crate::impl_registry_entry!(TextObjectEntry);

impl BuildEntry<TextObjectEntry> for TextObjectDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			aliases: self.meta.aliases,
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

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		let meta = self.meta_ref();
		sink.push(meta.id);
		sink.push(meta.name);
		sink.push(meta.description);
		for &alias in meta.aliases {
			sink.push(alias);
		}
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> TextObjectEntry {
		let meta_ref = self.meta_ref();
		let start = alias_pool.len() as u32;
		for &alias in meta_ref.aliases {
			alias_pool.push(interner.get(alias).expect("missing interned alias"));
		}
		let len = (alias_pool.len() as u32 - start) as u16;

		let meta = RegistryMeta {
			id: interner.get(meta_ref.id).expect("missing interned id"),
			name: interner.get(meta_ref.name).expect("missing interned name"),
			description: interner
				.get(meta_ref.description)
				.expect("missing interned description"),
			aliases: SymbolList { start, len },
			priority: meta_ref.priority,
			source: meta_ref.source,
			required_caps: CapabilitySet::from_iter(meta_ref.required_caps.iter().cloned()),
			flags: meta_ref.flags,
		};

		TextObjectEntry {
			meta,
			trigger: self.trigger,
			alt_triggers: self.alt_triggers,
			inner: self.inner,
			around: self.around,
		}
	}
}

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
	TEXT_OBJECTS.all()
}
