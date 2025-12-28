//! Action system for extensible commands and motions.
//!
//! Actions are the unified abstraction for all editor operations that can be
//! triggered by keybindings. This replaces the hardcoded `Command` enum with
//! a dynamic, extensible registry.
//!
//! # Typed Action Dispatch
//!
//! Actions use typed `ActionId` values for dispatch instead of strings:
//! - `ActionId` is a lightweight `u32` newtype assigned at registry build time
//! - Human-readable names are kept at the edges (config, help, keybindings)
//! - The input pipeline emits `ActionId` for fast, type-safe dispatch
//! - Name-to-ID resolution happens once during registry initialization

mod editing;
mod find;
mod insert;
mod misc;
mod modes;
mod motions;
mod scroll;
mod selection_ops;
mod text_objects;

// Re-export all action types from tome_manifest
/// Look up an action by name.
pub use tome_manifest::index::find_action;
pub use tome_manifest::{
	ACTIONS, ActionArgs, ActionContext, ActionDef, ActionHandler, ActionId, ActionMode,
	ActionResult, EditAction, ObjectSelectionKind, PendingAction, PendingKind, ScrollAmount,
	ScrollDir, VisualDirection,
};

/// Execute an action by name with the given context.
pub fn execute_action(name: &str, ctx: &ActionContext) -> ActionResult {
	match find_action(name) {
		Some(action) => (action.handler)(ctx),
		None => ActionResult::Error(format!("Unknown action: {}", name)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_find_action_unknown() {
		assert!(find_action("nonexistent_action_xyz").is_none());
	}

	#[test]
	fn test_motion_actions_registered() {
		assert!(find_action("move_left").is_some());
		assert!(find_action("move_right").is_some());
		assert!(find_action("move_up").is_some());
		assert!(find_action("move_down").is_some());
		assert!(find_action("move_line_start").is_some());
		assert!(find_action("move_line_end").is_some());
		assert!(find_action("next_word_start").is_some());
		assert!(find_action("document_start").is_some());
		assert!(find_action("document_end").is_some());
	}
}
