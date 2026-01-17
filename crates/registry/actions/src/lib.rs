//! Action registry definitions and handlers.
//!
//! Actions are the primary unit of editor functionality, executed via keybindings.
//! This module provides auto-registration via the [`action!`] macro and O(1) lookup.

use std::sync::LazyLock;

extern crate self as xeno_registry_actions;

pub use xeno_registry_core::{RegistryBuilder, RegistryReg, RuntimeRegistry};

/// Registry wrapper for action definitions.
pub struct ActionReg(pub &'static ActionDef);
inventory::collect!(ActionReg);

impl RegistryReg<ActionDef> for ActionReg {
	fn def(&self) -> &'static ActionDef {
		self.0
	}
}

mod context;
mod definition;
pub mod edit_op;
mod effects;
pub(crate) mod impls;
mod keybindings;
mod macros;
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
pub use impls::insert::insert_with_motion;
pub use impls::motions::{cursor_motion, selection_motion, word_motion};
pub use keybindings::{
	BindingMode, KEY_PREFIXES, KEYBINDINGS, KeyBindingDef, KeyBindingSetReg, KeyPrefixDef,
	KeyPrefixReg, find_prefix,
};
pub use pending::PendingAction;
pub use result::{
	ActionResult, RESULT_EFFECTS_HANDLERS, RESULT_EXTENSION_HANDLERS, ResultHandlerRegistry,
	ScreenPosition, dispatch_result, register_result_extension_handler, register_result_handler,
};
pub use xeno_primitives::direction::{Axis, SeqDirection, SpatialDirection};
pub use xeno_primitives::{Mode, ObjectSelectionKind, PendingKind};
pub use xeno_registry_core::{
	Capability, CommandError, RegistryEntry, RegistryMeta, RegistryMetadata, RegistrySource,
	impl_registry_entry,
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

/// Indexed collection of all actions with runtime registration support.
pub static ACTIONS: LazyLock<RuntimeRegistry<ActionDef>> = LazyLock::new(|| {
	let builtins = RegistryBuilder::new("actions")
		.extend_inventory::<ActionReg>()
		.sort_by(|a, b| a.meta.name.cmp(b.meta.name))
		.build();
	RuntimeRegistry::new("actions", builtins)
});

/// Registers an action definition at runtime.
///
/// Returns `true` if the action was added, `false` if already registered.
pub fn register_action(def: &'static ActionDef) -> bool {
	ACTIONS.register(def)
}

/// Finds an action by name, alias, or id.
pub fn find_action(name: &str) -> Option<&'static ActionDef> {
	ACTIONS.get(name)
}

/// Returns all registered actions (builtins + runtime), sorted by name.
pub fn all_actions() -> impl Iterator<Item = &'static ActionDef> {
	ACTIONS.all().into_iter()
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
