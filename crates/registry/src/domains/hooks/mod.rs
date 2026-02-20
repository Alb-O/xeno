//! Async hook system for editor events.

pub use crate::core::{HookId, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryRef, RuntimeRegistry};

#[path = "compile/builtins.rs"]
pub mod builtins;
#[path = "exec/context.rs"]
mod context;
mod domain;
#[path = "exec/emit.rs"]
mod emit;
#[path = "exec/handler.rs"]
pub mod handler;
#[path = "compile/link.rs"]
pub mod link;
#[path = "compile/loader.rs"]
pub mod loader;
#[path = "exec/macros.rs"]
mod macros;
#[path = "runtime/query.rs"]
pub mod query;
#[path = "contract/spec.rs"]
pub mod spec;
#[path = "contract/types.rs"]
mod types;

pub use builtins::register_builtins;
pub use domain::Hooks;
pub use query::HooksRegistry;

/// Registers compiled hooks from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_hooks_spec();
	let handlers = inventory::iter::<handler::HookHandlerReg>.into_iter().map(|r| r.0);

	let linked = link::link_hooks(&spec, handlers);

	for def in linked {
		db.push_domain::<Hooks>(HookInput::Linked(def));
	}
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
