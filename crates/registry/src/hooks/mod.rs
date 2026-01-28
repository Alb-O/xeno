//! Async hook system for editor events.
//!
//! Hooks allow extensions to react to editor events like file open, save,
//! mode change, etc.

pub use crate::core::{RegistryBuilder, RegistryEntry, RegistryIndex, RuntimeRegistry};

pub mod builtins;
mod context;
mod emit;
mod macros;
mod types;

pub use builtins::register_builtins;

pub fn register_plugin(db: &mut crate::db::builder::RegistryDbBuilder) {
	register_builtins(db);
}

inventory::submit! {
	crate::PluginDef::new(
		crate::RegistryMeta::minimal("hooks-builtin", "Hooks Builtin", "Builtin hook set"),
		register_plugin
	)
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
pub use crate::db::BUILTIN_HOOK_BY_EVENT;
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
	let mut hooks: Vec<_> = BUILTIN_HOOK_BY_EVENT
		.get(&event)
		.map(Vec::as_slice)
		.unwrap_or(&[])
		.to_vec();

	// Integrate runtime extensions matching this event
	hooks.extend(
		HOOKS
			.extras_items()
			.into_iter()
			.filter(|h| h.event == event),
	);

	// Ensure global consistency across builtin and runtime hooks
	hooks.sort_by(|a: &&HookDef, b: &&HookDef| a.total_order_cmp(b));

	hooks
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
