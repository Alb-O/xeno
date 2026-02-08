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

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
	}

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> MotionEntry {
		let meta =
			crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

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

/// Finds a motion by name or key.
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
pub type MotionInput =
	crate::core::def_input::DefInput<MotionDef, crate::kdl::link::LinkedMotionDef>;
