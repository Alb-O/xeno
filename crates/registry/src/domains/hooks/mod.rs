//! Async hook system for editor events.

pub use crate::core::{HookId, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryRef, RuntimeRegistry};

pub mod builtins;
mod context;
mod emit;
pub mod handler;
pub mod link;
pub mod loader;
mod macros;
pub mod registry;
pub mod spec;
mod types;

pub use builtins::register_builtins;
pub use registry::HooksRegistry;

/// Registers compiled hooks from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_hooks_spec();
	let handlers = inventory::iter::<handler::HookHandlerReg>.into_iter().map(|r| r.0);

	let linked = link::link_hooks(&spec, handlers);

	for def in linked {
		db.push_domain::<Hooks>(HookInput::Linked(def));
	}
}

pub struct Hooks;

impl crate::db::domain::DomainSpec for Hooks {
	type Input = HookInput;
	type Entry = HookEntry;
	type Id = crate::core::HookId;
	type Runtime = HooksRegistry;
	const LABEL: &'static str = "hooks";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.hooks
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		HooksRegistry::new(index)
	}

	fn on_push(_db: &mut crate::db::builder::RegistryDbBuilder, _input: &Self::Input) {}
}

pub use context::{Bool, HookContext, MutableHookContext, OptionViewId, SplitDirection, Str, ViewId, WindowId, WindowKind};
pub use emit::{HookScheduler, emit, emit_mutable, emit_sync, emit_sync_with};
pub use handler::{HookHandlerReg, HookHandlerStatic};
pub use types::{HookAction, HookDef, HookEntry, HookFuture, HookHandler, HookInput, HookMutability, HookPriority, HookResult};
pub use xeno_primitives::Mode;

#[cfg(feature = "minimal")]
pub use crate::db::HOOKS;
// Re-export macros
pub use crate::hook_handler;

pub type HooksRef = RegistryRef<HookEntry, HookId>;

#[cfg(feature = "minimal")]
pub fn hooks_for_event(event: crate::HookEvent) -> Vec<HooksRef> {
	HOOKS.for_event(event)
}

#[cfg(feature = "minimal")]
pub fn find_hooks(event: crate::HookEvent) -> Vec<HooksRef> {
	hooks_for_event(event)
}

/// Returns all registered hooks.
#[cfg(feature = "minimal")]
pub fn all_hooks() -> Vec<HooksRef> {
	HOOKS.snapshot_guard().iter_refs().collect()
}
