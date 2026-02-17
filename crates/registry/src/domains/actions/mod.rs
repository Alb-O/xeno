//! Action registry definitions and handlers.
//!
//! Actions are the primary unit of editor functionality, executed via keybindings.
//! This module provides auto-registration via the [`action_handler!`] macro and O(1) lookup.

pub use crate::core::{
	Capability, CommandError, RegistryBuilder, RegistryEntry, RegistryMeta, RegistryMetaStatic, RegistryMetadata, RegistryRef, RegistrySource, RuntimeRegistry,
};

pub mod builtins;
mod context;
pub mod def;
pub mod edit_op;
mod effects;
pub mod entry;
pub mod handler;
mod keybindings;
pub mod link;
pub mod loader;
mod macros;
mod pending;
mod result;
pub mod spec;

pub mod editor_ctx;
pub use context::{ActionArgs, ActionContext};
pub use def::{ActionDef, ActionHandler};
pub use editor_ctx::{
	CommandQueueAccess, CursorAccess, EditAccess, EditorCapabilities, EditorContext, EditorOps, FileOpsAccess, FocusOps, HandleOutcome, JumpAccess,
	MacroAccess, ModeAccess, MotionAccess, MotionDispatchAccess, NotificationAccess, OptionAccess, PaletteAccess, ResultHandler, SearchAccess, SelectionAccess,
	SplitOps, TextAccess, ThemeAccess, UndoAccess, ViewportAccess,
};
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
pub use edit_op::{CharMapKind, CursorAdjust, EditOp, EditPlan, PostEffect, PreEffect, SelectionOp, TextTransform};
pub use effects::{ActionEffects, AppEffect, EditEffect, Effect, MotionKind, MotionRequest, ScrollAmount, UiEffect, ViewEffect};
pub use keybindings::{BindingMode, KEY_PREFIXES, KeyBindingDef, KeyPrefixDef, find_prefix};
pub use pending::PendingAction;
pub use result::{
	ActionResult, RESULT_EFFECTS_HANDLERS, RESULT_EXTENSION_HANDLERS, ResultHandlerRegistry, ScreenPosition, dispatch_result,
	register_result_extension_handler, register_result_handler,
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

#[cfg(feature = "minimal")]
pub use crate::db::ACTIONS;
use crate::error::RegistryError;

pub fn register_plugin(db: &mut crate::db::builder::RegistryDbBuilder) -> Result<(), RegistryError> {
	register_builtins(db);
	register_compiled(db);
	Ok(())
}

/// Registers compiled actions and prefixes from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_actions_spec();
	let handlers = inventory::iter::<handler::ActionHandlerReg>.into_iter().map(|r| r.0);

	let linked = link::link_actions(&spec, handlers);

	for def in linked {
		db.push_domain::<Actions>(def::ActionInput::Linked(def));
	}

	db.register_key_prefixes(link::link_prefixes(&spec));
}

pub struct Actions;

impl crate::db::domain::DomainSpec for Actions {
	type Input = def::ActionInput;
	type Entry = entry::ActionEntry;
	type Id = crate::core::ActionId;
	type StaticDef = def::ActionDef;
	type LinkedDef = link::LinkedActionDef;
	const LABEL: &'static str = "actions";

	fn static_to_input(def: &'static Self::StaticDef) -> Self::Input {
		def::ActionInput::Static(def.clone())
	}

	fn linked_to_input(def: Self::LinkedDef) -> Self::Input {
		def::ActionInput::Linked(def)
	}

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.actions
	}

	fn on_push(_db: &mut crate::db::builder::RegistryDbBuilder, _input: &Self::Input) {}
}

/// Registers an action definition at runtime.
///
/// Returns `true` if the action was added, `false` if rejected (e.g., lower priority duplicate).
#[cfg(feature = "minimal")]
pub fn register_action(def: &'static ActionDef) -> bool {
	ACTIONS.register(def).is_ok()
}

/// Finds an action by name, key, or id.
#[cfg(feature = "minimal")]
pub fn find_action(name: &str) -> Option<ActionRef> {
	ACTIONS.get(name)
}

/// Returns all registered actions (builtins + runtime), sorted by name.
#[cfg(feature = "minimal")]
pub fn all_actions() -> Vec<ActionRef> {
	ACTIONS.snapshot_guard().iter_refs().collect()
}
