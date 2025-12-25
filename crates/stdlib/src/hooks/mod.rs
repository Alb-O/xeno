//! Hook implementations.
//!
//! This module contains the built-in hook implementations.
//! Type definitions are in tome-manifest.

mod log_buffer_open;
mod log_mode_change;

// Re-export types from tome-manifest for use in hook implementations
pub use tome_manifest::Mode;
pub use tome_manifest::hooks::{
	HOOKS, HookContext, HookDef, HookEvent, HookResult, MUTABLE_HOOKS, MutableHookContext,
	MutableHookDef, all_hooks, emit, emit_mutable, find_hooks,
};
