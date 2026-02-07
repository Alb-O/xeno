//! Motion registry.

use ropey::RopeSlice;
use xeno_primitives::Range;

use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
pub use crate::core::{
	Capability, CapabilitySet, FrozenInterner, MotionId, RegistryBuilder, RegistryEntry,
	RegistryIndex, RegistryMeta, RegistryMetaStatic, RegistryMetadata, RegistryRef, RegistrySource,
	RuntimeRegistry, Symbol, SymbolList,
};

#[macro_use]
pub(crate) mod macros;

pub mod builtins;
pub mod handler;
pub mod movement;

pub use builtins::register_builtins;
pub use handler::{MotionHandlerReg, MotionHandlerStatic};

use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

// Re-export macros
pub use crate::motion_handler;

/// Command flags for motion definitions.
pub mod flags {
	/// No flags set.
	pub const NONE: u32 = 0;
}

/// Handler signature for motion primitives.
pub type MotionHandler = fn(RopeSlice, Range, usize, bool) -> Range;

/// Definition of a motion primitive (static input for builder).
#[derive(Clone)]
pub struct MotionDef {
	/// Common registry metadata (static).
	pub meta: RegistryMetaStatic,
	/// Function that implements the motion logic.
	pub handler: MotionHandler,
}

/// Symbolized motion entry stored in the registry snapshot.
pub struct MotionEntry {
	/// Common registry metadata (symbolized).
	pub meta: RegistryMeta,
	/// Function that implements the motion logic.
	pub handler: MotionHandler,
}

crate::impl_registry_entry!(MotionEntry);

impl BuildEntry<MotionEntry> for MotionDef {
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

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> MotionEntry {
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

		MotionEntry {
			meta,
			handler: self.handler,
		}
	}
}

/// Typed reference to a runtime motion entry.
pub type MotionRef = RegistryRef<MotionEntry, MotionId>;

#[cfg(feature = "db")]
pub use crate::db::MOTIONS;

/// Finds a motion by name or alias.
#[cfg(feature = "db")]
pub fn find(name: &str) -> Option<MotionRef> {
	MOTIONS.get(name)
}

/// Returns all registered motions, sorted by name.
#[cfg(feature = "db")]
pub fn all() -> Vec<MotionRef> {
	MOTIONS.all()
}

/// Unified motion input: either a static `MotionDef` or a KDL-linked definition.
pub enum MotionInput {
	/// Static definition from `motion!` macro.
	Static(MotionDef),
	/// KDL-linked definition with owned metadata.
	Linked(crate::kdl::link::LinkedMotionDef),
}

impl BuildEntry<MotionEntry> for MotionInput {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		match self {
			Self::Static(def) => def.meta_ref(),
			Self::Linked(def) => def.meta_ref(),
		}
	}

	fn short_desc_str(&self) -> &str {
		match self {
			Self::Static(def) => def.short_desc_str(),
			Self::Linked(def) => def.short_desc_str(),
		}
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		match self {
			Self::Static(def) => def.collect_strings(sink),
			Self::Linked(def) => def.collect_strings(sink),
		}
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> MotionEntry {
		match self {
			Self::Static(def) => def.build(interner, alias_pool),
			Self::Linked(def) => def.build(interner, alias_pool),
		}
	}
}
