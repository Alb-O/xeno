//! Text object registry.

use std::sync::Arc;

use ropey::RopeSlice;
use xeno_primitives::Range;

pub mod builtins;
pub mod handler;
mod macros;
pub mod registry;

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

use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
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
			aliases: StrListRef::Static(self.meta.aliases),
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
		meta.aliases.for_each(|a| sink.push(a));
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> TextObjectEntry {
		let meta_ref = self.meta_ref();
		let start = alias_pool.len() as u32;
		meta_ref.aliases.for_each(|alias| {
			alias_pool.push(interner.get(alias).expect("missing interned alias"));
		});
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
			alt_triggers: Arc::from(self.alt_triggers),
			inner: self.inner,
			around: self.around,
		}
	}
}

/// Unified input for text object registration — either a static `TextObjectDef`
/// or a `LinkedTextObjectDef` assembled from KDL metadata + Rust handlers.
pub enum TextObjectInput {
	Static(TextObjectDef),
	Linked(crate::kdl::link::LinkedTextObjectDef),
}

impl BuildEntry<TextObjectEntry> for TextObjectInput {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		match self {
			Self::Static(d) => d.meta_ref(),
			Self::Linked(d) => d.meta_ref(),
		}
	}

	fn short_desc_str(&self) -> &str {
		match self {
			Self::Static(d) => d.short_desc_str(),
			Self::Linked(d) => d.short_desc_str(),
		}
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		match self {
			Self::Static(d) => d.collect_strings(sink),
			Self::Linked(d) => d.collect_strings(sink),
		}
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> TextObjectEntry {
		match self {
			Self::Static(d) => d.build(interner, alias_pool),
			Self::Linked(d) => d.build(interner, alias_pool),
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
