//! Action registry definitions and handlers.
//!
//! Actions are the primary unit of editor functionality, executed via keybindings.
//! This module provides auto-registration via the [`action!`] macro and O(1) lookup.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};

extern crate self as xeno_registry_actions;

/// Wrapper for [`inventory`] collection of action definitions.
pub struct ActionReg(pub &'static ActionDef);
inventory::collect!(ActionReg);

mod context;
mod definition;
pub mod edit_op;
mod effects;
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
	BindingMode, KEY_PREFIXES, KEYBINDINGS, KeyBindingDef, KeyBindingSetReg, KeyPrefixDef,
	KeyPrefixReg, find_prefix,
};
pub use motion_helpers::{cursor_motion, insert_with_motion, selection_motion, word_motion};
pub use pending::PendingAction;
pub use result::{
	ActionResult, RESULT_EFFECTS_HANDLERS, RESULT_EXTENSION_HANDLERS, ResultHandlerRegistry,
	ScreenPosition, dispatch_result, register_result_extension_handler, register_result_handler,
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

/// Runtime-registered actions (plugins, user extensions).
static EXTRA_ACTIONS: OnceLock<Mutex<Vec<&'static ActionDef>>> = OnceLock::new();

/// O(1) action lookup index, keyed by id, name, and aliases.
static ACTION_INDEX: LazyLock<Mutex<HashMap<&'static str, &'static ActionDef>>> =
	LazyLock::new(|| {
		let mut map = HashMap::new();
		for reg in inventory::iter::<ActionReg> {
			insert_action_into_index(&mut map, reg.0);
		}
		Mutex::new(map)
	});

fn insert_action_into_index(
	map: &mut HashMap<&'static str, &'static ActionDef>,
	def: &'static ActionDef,
) {
	map.insert(def.id(), def);
	map.insert(def.name(), def);
	for &alias in def.aliases() {
		map.insert(alias, def);
	}
}

/// Registers an action definition at runtime.
///
/// This is a no-op if the action is already registered via inventory or a previous
/// call to this function.
pub fn register_action(def: &'static ActionDef) {
	if inventory::iter::<ActionReg>().any(|reg| std::ptr::eq(reg.0, def)) {
		return;
	}

	let mut extras = EXTRA_ACTIONS
		.get_or_init(|| Mutex::new(Vec::new()))
		.lock()
		.expect("extra action lock poisoned");

	if extras.iter().any(|&existing| std::ptr::eq(existing, def)) {
		return;
	}
	extras.push(def);
	drop(extras);

	let mut index = ACTION_INDEX.lock().expect("action index lock poisoned");
	insert_action_into_index(&mut index, def);
}

/// Finds an action by name, alias, or id.
pub fn find_action(name: &str) -> Option<&'static ActionDef> {
	ACTION_INDEX.lock().expect("poisoned").get(name).copied()
}

/// Returns all registered actions, sorted by name.
pub fn all_actions() -> impl Iterator<Item = &'static ActionDef> {
	let mut actions: Vec<_> = inventory::iter::<ActionReg>().map(|r| r.0).collect();

	if let Some(extras) = EXTRA_ACTIONS.get() {
		actions.extend(extras.lock().expect("poisoned").iter().copied());
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
