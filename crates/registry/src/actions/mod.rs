//! Action registry definitions and handlers.
//!
//! Actions are the primary unit of editor functionality, executed via keybindings.
//! This module provides auto-registration via the [`action_handler!`] macro and O(1) lookup.

pub use crate::core::{
	Capability, CommandError, RegistryBuilder, RegistryEntry, RegistryMeta, RegistryMetaStatic,
	RegistryMetadata, RegistryRef, RegistrySource, RuntimeRegistry,
};

pub mod builtins;
mod context;
pub mod def;
pub mod edit_op;
mod effects;
pub mod entry;
pub mod handler;
mod keybindings;
mod macros;
mod pending;
mod result;

pub mod editor_ctx;

pub use context::{ActionArgs, ActionContext};
pub use def::{ActionDef, ActionHandler};
pub use entry::ActionEntry;
pub use handler::{ActionHandlerReg, ActionHandlerStatic};

// Re-export macros
pub use crate::action_handler;
pub use crate::core::{ActionId, LookupKey};
pub use crate::{result_extension_handler, result_handler};

/// Typed handle for looking up an action by canonical ID or registry reference.
pub type ActionKey = LookupKey<ActionEntry, ActionId>;
/// Typed reference to a runtime action entry.
pub type ActionRef = RegistryRef<ActionEntry, ActionId>;
pub use builtins::{cursor_motion, selection_motion};
pub use edit_op::{
	CharMapKind, CursorAdjust, EditOp, EditPlan, PostEffect, PreEffect, SelectionOp, TextTransform,
};
pub use effects::{
	ActionEffects, AppEffect, EditEffect, Effect, MotionKind, MotionRequest, ScrollAmount,
	UiEffect, ViewEffect,
};
pub use keybindings::{BindingMode, KEY_PREFIXES, KeyBindingDef, KeyPrefixDef, find_prefix};
pub use pending::PendingAction;
pub use result::{
	ActionResult, RESULT_EFFECTS_HANDLERS, RESULT_EXTENSION_HANDLERS, ResultHandlerRegistry,
	ScreenPosition, dispatch_result, register_result_extension_handler, register_result_handler,
};
pub use xeno_primitives::direction::{Axis, SeqDirection, SpatialDirection};
pub use xeno_primitives::{Mode, ObjectSelectionKind, PendingKind};

/// Command flags for action definitions.
pub mod flags {
	/// No flags set.
	pub const NONE: u32 = 0;
}

/// Typed handles for built-in actions.
pub mod keys {
	pub use super::builtins::editing::*;
	pub use super::builtins::find::*;
	pub use super::builtins::insert::*;
	pub use super::builtins::misc::*;
	pub use super::builtins::modes::*;
	pub use super::builtins::navigation::*;
	pub use super::builtins::scrolling::*;
	pub use super::builtins::search::*;
	pub use super::builtins::selection::*;
	pub use super::builtins::text_objects::*;
	pub use super::builtins::window::*;
}

pub use builtins::register_builtins;

#[cfg(feature = "db")]
pub use crate::db::ACTIONS;
use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

/// Registers an action definition at runtime.
///
/// Returns `true` if the action was added, `false` if rejected (e.g., lower priority duplicate).
#[cfg(feature = "db")]
pub fn register_action(def: &'static ActionDef) -> bool {
	ACTIONS.register(def).is_ok()
}

/// Finds an action by name, alias, or id.
#[cfg(feature = "db")]
pub fn find_action(name: &str) -> Option<ActionRef> {
	ACTIONS.get(name)
}

/// Returns all registered actions (builtins + runtime), sorted by name.
#[cfg(feature = "db")]
pub fn all_actions() -> Vec<ActionRef> {
	ACTIONS.all()
}
