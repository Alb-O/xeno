//! Hook implementations.
//!
//! This module contains the built-in hook implementations.
//! Type definitions are in evildoer-manifest.

mod log_buffer_open;
mod log_mode_change;

// Re-export types from evildoer-manifest for use in hook implementations
pub use evildoer_manifest::Mode;
pub use evildoer_manifest::hooks::{
	BoxFuture, HOOKS, HookAction, HookContext, HookDef, HookEvent, HookEventData, HookResult,
	MUTABLE_HOOKS, MutableHookContext, MutableHookDef, all_hooks, emit, emit_mutable, emit_sync,
	find_hooks,
};
