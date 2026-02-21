//! Motion registry.

use ropey::RopeSlice;
use xeno_primitives::Range;

use crate::core::index::{BuildCtx, BuildEntry, RegistryMetaRef, StrListRef};
pub use crate::core::{
	FrozenInterner, MotionId, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta, RegistryMetaStatic, RegistryMetadata, RegistryRef, RegistrySource,
	RuntimeRegistry, Symbol, SymbolList,
};

#[macro_use]
#[path = "exec/macros.rs"]
pub(crate) mod macros;

#[path = "compile/builtins.rs"]
pub mod builtins;
mod domain;
#[path = "exec/handler.rs"]
pub mod handler;
#[path = "compile/link.rs"]
pub mod link;
#[path = "compile/loader.rs"]
pub mod loader;
#[path = "exec/movement/mod.rs"]
pub mod movement;
#[path = "contract/spec.rs"]
pub mod spec;

pub use builtins::register_builtins;
pub use domain::Motions;
pub use handler::{MotionHandlerReg, MotionHandlerStatic};

/// Registers compiled motions from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_motions_spec();
	let handlers = inventory::iter::<handler::MotionHandlerReg>.into_iter().map(|r| r.0);

	let linked = link::link_motions(&spec, handlers);

	for def in linked {
		db.push_domain::<Motions>(MotionInput::Linked(def));
	}
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
			mutates_buffer: self.meta.mutates_buffer,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name
	}

	fn collect_payload_strings<'b>(&'b self, _collector: &mut crate::core::index::StringCollector<'_, 'b>) {}

	fn build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> MotionEntry {
		let meta = crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), []);

		MotionEntry { meta, handler: self.handler }
	}
}

/// Typed reference to a runtime motion entry.
pub type MotionRef = RegistryRef<MotionEntry, MotionId>;

#[cfg(feature = "minimal")]
pub use crate::db::MOTIONS;

/// Finds a motion by name or key.
#[cfg(feature = "minimal")]
pub fn find(name: &str) -> Option<MotionRef> {
	MOTIONS.get(name)
}

/// Returns all registered motions, sorted by name.
#[cfg(feature = "minimal")]
pub fn all() -> Vec<MotionRef> {
	MOTIONS.snapshot_guard().iter_refs().collect()
}

/// Unified motion input: either a static `MotionDef` or a registry-linked definition.
pub type MotionInput = crate::core::def_input::DefInput<MotionDef, crate::motions::link::LinkedMotionDef>;
