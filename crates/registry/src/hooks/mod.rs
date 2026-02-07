//! Async hook system for editor events.

pub use crate::core::{
	HookId, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryRef, RuntimeRegistry,
};

pub mod builtins;
mod context;
mod emit;
mod macros;
pub mod registry;
mod types;

pub use builtins::register_builtins;
pub use registry::HooksRegistry;

use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

pub use context::{
	Bool, HookContext, MutableHookContext, OptionViewId, SplitDirection, Str, ViewId, WindowId,
	WindowKind,
};
pub use emit::{HookScheduler, emit, emit_mutable, emit_sync, emit_sync_with};
pub use types::{
	HookAction, HookDef, HookEntry, HookFuture, HookHandler, HookMutability, HookPriority,
	HookResult,
};
pub use xeno_primitives::Mode;

pub use crate::async_hook;
#[cfg(feature = "db")]
pub use crate::db::HOOKS;
// Re-export macros
pub use crate::hook;

pub type HooksRef = RegistryRef<HookEntry, HookId>;

#[cfg(feature = "db")]
pub fn register_hook(def: &'static HookDef) -> bool {
	HOOKS.register(def)
}

#[cfg(feature = "db")]
pub fn hooks_for_event(event: crate::HookEvent) -> Vec<HooksRef> {
	HOOKS.for_event(event)
}

#[cfg(feature = "db")]
pub fn find_hooks(event: crate::HookEvent) -> Vec<HooksRef> {
	hooks_for_event(event)
}

#[cfg(feature = "db")]
pub fn all_hooks() -> Vec<HooksRef> {
	HOOKS.all()
}
