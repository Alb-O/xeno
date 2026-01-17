//! Action registry definitions and handlers.
//!
//! Actions are defined in static lists and executed via keybindings.

use std::sync::{Mutex, OnceLock};

extern crate self as xeno_registry_actions;

mod context;
mod definition;
pub mod edit_op;
mod effects;
/// Built-in action implementations.
pub(crate) mod impls;
mod keybindings;
mod macros;
mod motion_helpers;
mod pending;
mod result;

pub mod editor_ctx;

pub use context::{ActionArgs, ActionContext};
pub use definition::{ActionDef, ActionHandler};
pub use xeno_registry_core::Key;

/// Typed handle to an action definition.
pub type ActionKey = Key<ActionDef>;
pub use edit_op::{
	CharMapKind, CursorAdjust, EditOp, EditPlan, PostEffect, PreEffect, SelectionOp, TextTransform,
};
pub use effects::{
	ActionEffects, AppEffect, EditEffect, Effect, ScrollAmount, UiEffect, ViewEffect,
};
pub use keybindings::{
	BindingMode, KEY_PREFIXES, KEYBINDINGS, KeyBindingDef, KeyPrefixDef, find_prefix,
};
pub use motion_helpers::{cursor_motion, insert_with_motion, selection_motion, word_motion};
pub use pending::PendingAction;
pub use result::{
	ActionResult, RESULT_EFFECTS_HANDLERS, RESULT_EXTENSION_HANDLERS, ScreenPosition,
	dispatch_result,
};
pub use xeno_primitives::direction::{Axis, SeqDirection, SpatialDirection};
pub use xeno_primitives::{Mode, ObjectSelectionKind, PendingKind};
pub use xeno_registry_commands::CommandError;
pub use xeno_registry_core::{
	Capability, RegistryEntry, RegistryMeta, RegistryMetadata, RegistrySource, impl_registry_entry,
};
pub use xeno_registry_motions::flags;

/// Typed handles for built-in actions.
pub mod keys {
	pub use crate::impls::editing::*;
	pub use crate::impls::find::*;
	pub use crate::impls::insert::*;
	pub use crate::impls::misc::*;
	pub use crate::impls::modes::*;
	pub use crate::impls::motions::*;
	pub use crate::impls::palette::*;
	pub use crate::impls::scroll::*;
	pub use crate::impls::selection_ops::*;
	pub use crate::impls::text_objects::*;
	pub use crate::impls::window::*;
}

/// Registry of all action definitions.
pub static ACTIONS: &[&ActionDef] = &[
	&impls::editing::ACTION_delete,
	&impls::editing::ACTION_delete_no_yank,
	&impls::editing::ACTION_change,
	&impls::editing::ACTION_change_no_yank,
	&impls::editing::ACTION_yank,
	&impls::editing::ACTION_paste_after,
	&impls::editing::ACTION_paste_before,
	&impls::editing::ACTION_paste_all_after,
	&impls::editing::ACTION_paste_all_before,
	&impls::editing::ACTION_undo,
	&impls::editing::ACTION_redo,
	&impls::editing::ACTION_indent,
	&impls::editing::ACTION_deindent,
	&impls::editing::ACTION_to_lowercase,
	&impls::editing::ACTION_to_uppercase,
	&impls::editing::ACTION_swap_case,
	&impls::editing::ACTION_join_lines,
	&impls::editing::ACTION_open_below,
	&impls::editing::ACTION_open_above,
	&impls::editing::ACTION_delete_back,
	&impls::editing::ACTION_delete_word_back,
	&impls::editing::ACTION_delete_word_forward,
	&impls::editing::ACTION_replace_char,
	&impls::find::ACTION_find_char,
	&impls::find::ACTION_find_char_to,
	&impls::find::ACTION_find_char_reverse,
	&impls::find::ACTION_find_char_to_reverse,
	&impls::insert::ACTION_insert_mode,
	&impls::insert::ACTION_insert_line_start,
	&impls::insert::ACTION_insert_line_end,
	&impls::insert::ACTION_insert_after,
	&impls::insert::ACTION_insert_newline,
	&impls::misc::ACTION_add_line_below,
	&impls::misc::ACTION_add_line_above,
	&impls::misc::ACTION_use_selection_as_search,
	&impls::modes::ACTION_normal_mode,
	&impls::motions::ACTION_move_left,
	&impls::motions::ACTION_move_right,
	&impls::motions::ACTION_move_line_start,
	&impls::motions::ACTION_move_line_end,
	&impls::motions::ACTION_next_word_start,
	&impls::motions::ACTION_prev_word_start,
	&impls::motions::ACTION_next_word_end,
	&impls::motions::ACTION_next_long_word_start,
	&impls::motions::ACTION_prev_long_word_start,
	&impls::motions::ACTION_next_long_word_end,
	&impls::motions::ACTION_select_word_forward,
	&impls::motions::ACTION_select_word_backward,
	&impls::motions::ACTION_select_word_end,
	&impls::motions::ACTION_next_paragraph,
	&impls::motions::ACTION_prev_paragraph,
	&impls::motions::ACTION_document_start,
	&impls::motions::ACTION_document_end,
	&impls::motions::ACTION_goto_line_start,
	&impls::motions::ACTION_goto_line_end,
	&impls::motions::ACTION_goto_first_nonwhitespace,
	&impls::motions::ACTION_move_top_screen,
	&impls::motions::ACTION_move_middle_screen,
	&impls::motions::ACTION_move_bottom_screen,
	&impls::palette::ACTION_open_palette,
	&impls::scroll::ACTION_scroll_up,
	&impls::scroll::ACTION_scroll_down,
	&impls::scroll::ACTION_scroll_half_page_up,
	&impls::scroll::ACTION_scroll_half_page_down,
	&impls::scroll::ACTION_scroll_page_up,
	&impls::scroll::ACTION_scroll_page_down,
	&impls::scroll::ACTION_move_up_visual,
	&impls::scroll::ACTION_move_down_visual,
	&impls::selection_ops::ACTION_collapse_selection,
	&impls::selection_ops::ACTION_flip_selection,
	&impls::selection_ops::ACTION_ensure_forward,
	&impls::selection_ops::ACTION_select_line,
	&impls::selection_ops::ACTION_select_all,
	&impls::selection_ops::ACTION_expand_to_line,
	&impls::selection_ops::ACTION_remove_primary_selection,
	&impls::selection_ops::ACTION_remove_selections_except_primary,
	&impls::selection_ops::ACTION_rotate_selections_forward,
	&impls::selection_ops::ACTION_rotate_selections_backward,
	&impls::selection_ops::ACTION_split_lines,
	&impls::selection_ops::ACTION_duplicate_selections_down,
	&impls::selection_ops::ACTION_duplicate_selections_up,
	&impls::selection_ops::ACTION_merge_selections,
	&impls::text_objects::ACTION_select_object_inner,
	&impls::text_objects::ACTION_select_object_around,
	&impls::text_objects::ACTION_select_object_to_start,
	&impls::text_objects::ACTION_select_object_to_end,
	&impls::window::ACTION_split_horizontal,
	&impls::window::ACTION_split_vertical,
	&impls::window::ACTION_focus_left,
	&impls::window::ACTION_focus_down,
	&impls::window::ACTION_focus_up,
	&impls::window::ACTION_focus_right,
	&impls::window::ACTION_buffer_next,
	&impls::window::ACTION_buffer_prev,
	&impls::window::ACTION_close_split,
	&impls::window::ACTION_close_other_buffers,
];

static EXTRA_ACTIONS: OnceLock<Mutex<Vec<&'static ActionDef>>> = OnceLock::new();

/// Registers an extra action definition at runtime.
pub fn register_action(def: &'static ActionDef) {
	let mut extras = EXTRA_ACTIONS
		.get_or_init(|| Mutex::new(Vec::new()))
		.lock()
		.expect("extra action lock poisoned");

	if ACTIONS.iter().any(|&existing| std::ptr::eq(existing, def)) {
		return;
	}

	if extras.iter().any(|&existing| std::ptr::eq(existing, def)) {
		return;
	}

	extras.push(def);
}

/// Find an action by name or alias.
pub fn find_action(name: &str) -> Option<&'static ActionDef> {
	if let Some(action) = ACTIONS
		.iter()
		.copied()
		.find(|action| action.name() == name || action.aliases().contains(&name))
	{
		return Some(action);
	}

	let extras = EXTRA_ACTIONS.get()?;
	extras
		.lock()
		.expect("extra action lock poisoned")
		.iter()
		.copied()
		.find(|action| action.name() == name || action.aliases().contains(&name))
}

/// Returns all registered actions, sorted by name.
pub fn all_actions() -> impl Iterator<Item = &'static ActionDef> {
	let mut actions: Vec<_> = ACTIONS.iter().copied().collect();
	if let Some(extras) = EXTRA_ACTIONS.get() {
		actions.extend(
			extras
				.lock()
				.expect("extra action lock poisoned")
				.iter()
				.copied(),
		);
	}
	actions.sort_by_key(|action| action.name());
	actions.into_iter()
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
		assert!(find_action("move_up_visual").is_some());
		assert!(find_action("move_down_visual").is_some());
		assert!(find_action("move_line_start").is_some());
		assert!(find_action("move_line_end").is_some());
		assert!(find_action("next_word_start").is_some());
		assert!(find_action("document_start").is_some());
		assert!(find_action("document_end").is_some());
	}
}
