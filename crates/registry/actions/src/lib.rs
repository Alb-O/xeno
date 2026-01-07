//! Action registry definitions and handlers.
//!
//! Actions are registered at compile time via [`linkme`] distributed slices
//! and executed via keybindings.

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
	CharMapKind, CursorAdjust, EditOp, PostEffect, PreEffect, SelectionOp, TextTransform,
};
pub use effects::{ActionEffects, Effect, ScrollAmount};
pub use keybindings::{
	BindingMode, KEY_PREFIXES, KEYBINDINGS, KeyBindingDef, KeyPrefixDef, find_prefix,
};
use linkme::distributed_slice;
pub use motion_helpers::{cursor_motion, insert_with_motion, selection_motion, word_motion};
pub use pending::PendingAction;
pub use result::{
	ActionResult, RESULT_EFFECTS_HANDLERS, RESULT_EXTENSION_HANDLERS, ScreenPosition,
	dispatch_result,
};
pub use xeno_base::direction::{Axis, SeqDirection, SpatialDirection};
pub use xeno_base::{Mode, ObjectSelectionKind, PendingKind};
pub use xeno_registry_commands::CommandError;
pub use xeno_registry_core::{RegistryMetadata, RegistrySource, impl_registry_metadata};
pub use xeno_registry_motions::{Capability, flags};

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
#[distributed_slice]
pub static ACTIONS: [ActionDef];

/// Find an action by name or alias.
pub fn find_action(name: &str) -> Option<&'static ActionDef> {
	ACTIONS
		.iter()
		.find(|action| action.name == name || action.aliases.contains(&name))
}

/// Returns all registered actions, sorted by name.
pub fn all_actions() -> impl Iterator<Item = &'static ActionDef> {
	let mut actions: Vec<_> = ACTIONS.iter().collect();
	actions.sort_by_key(|action| action.name);
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
