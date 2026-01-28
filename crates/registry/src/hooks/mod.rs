//! Async hook system for editor events.
//!
//! Hooks allow extensions to react to editor events like file open, save,
//! mode change, etc.

pub use crate::core::{RegistryBuilder, RegistryEntry, RegistryIndex, RuntimeRegistry};

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
	BoxFuture, HookAction, HookDef, HookHandler, HookMutability, HookPriority, HookResult,
};
pub use xeno_primitives::Mode;

pub use crate::async_hook;
#[cfg(feature = "db")]
pub use crate::db::HOOKS;
// Re-export macros
pub use crate::hook;

/// Registers a new hook definition at runtime.
#[cfg(feature = "db")]
pub fn register_hook(def: &'static HookDef) -> bool {
	HOOKS.register(def)
}

/// Returns all hooks registered for the given `event`, in execution order.
#[cfg(feature = "db")]
pub fn hooks_for_event(event: crate::HookEvent) -> Vec<&'static HookDef> {
	HOOKS.for_event(event)
}

/// Find all hooks registered for a specific event.
#[cfg(feature = "db")]
pub fn find_hooks(event: crate::HookEvent) -> impl Iterator<Item = &'static HookDef> {
	hooks_for_event(event).into_iter()
}

/// List all registered hooks (builtins + runtime).
#[cfg(feature = "db")]
pub fn all_hooks() -> impl Iterator<Item = &'static HookDef> {
	HOOKS.all().into_iter()
}
