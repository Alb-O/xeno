//! Action registry definitions and handlers.
//!
//! Actions are the primary unit of editor functionality, executed via keybindings.
//! This module provides auto-registration via the [`action!`] macro and O(1) lookup.

pub use xeno_registry_core::{RegistryBuilder, RuntimeRegistry};

pub(crate) mod builtins;
mod context;
mod definition;
pub mod edit_op;
mod effects;
mod keybindings;
mod macros;
mod pending;
mod result;

pub mod editor_ctx;

pub use context::{ActionArgs, ActionContext};
pub use definition::{ActionDef, ActionHandler};
pub use xeno_registry_core::Key;

// Re-export macros for module-local usage
pub use crate::action;
pub use crate::{key_prefix, result_extension_handler, result_handler};

/// Typed handle to an action definition.
pub type ActionKey = Key<ActionDef>;
pub use builtins::insert::insert_with_motion;
pub use builtins::motions::{cursor_motion, selection_motion, word_motion};
pub use edit_op::{
	CharMapKind, CursorAdjust, EditOp, EditPlan, PostEffect, PreEffect, SelectionOp, TextTransform,
};
pub use effects::{
	ActionEffects, AppEffect, EditEffect, Effect, MotionKind, MotionRequest, ScrollAmount,
	UiEffect, ViewEffect,
};
pub use keybindings::{
	BindingMode, KEY_PREFIXES, KEYBINDINGS, KeyBindingDef, KeyPrefixDef, find_prefix,
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
/// Command flags for action definitions.
pub mod flags {
	/// No flags set.
	pub const NONE: u32 = 0;
}

/// Typed handles for built-in actions.
pub mod keys {
	pub use crate::actions::builtins::editing::*;
	pub use crate::actions::builtins::find::*;
	pub use crate::actions::builtins::insert::*;
	pub use crate::actions::builtins::misc::*;
	pub use crate::actions::builtins::modes::*;
	pub use crate::actions::builtins::motions::*;
	pub use crate::actions::builtins::palette::*;
	pub use crate::actions::builtins::scroll::*;
	pub use crate::actions::builtins::search::*;
	pub use crate::actions::builtins::selection_ops::*;
	pub use crate::actions::builtins::text_objects::*;
	pub use crate::actions::builtins::window::*;
}

#[cfg(feature = "db")]
pub use crate::db::ACTIONS;

/// Registers an action definition at runtime.
///
/// Returns `true` if the action was added, `false` if already registered.
#[cfg(feature = "db")]
pub fn register_action(def: &'static ActionDef) -> bool {
	ACTIONS.register(def)
}

/// Finds an action by name, alias, or id.
#[cfg(feature = "db")]
pub fn find_action(name: &str) -> Option<&'static ActionDef> {
	ACTIONS.get(name)
}

/// Returns all registered actions (builtins + runtime), sorted by name.
#[cfg(feature = "db")]
pub fn all_actions() -> impl Iterator<Item = &'static ActionDef> {
	ACTIONS.all().into_iter()
}
