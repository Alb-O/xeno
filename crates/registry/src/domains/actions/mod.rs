//! Action registry definitions and handlers.
//!
//! Actions are the primary unit of editor functionality, executed via keybindings.
//! This module provides auto-registration via the [`action_handler!`] macro and O(1) lookup.

pub use crate::core::{CommandError, RegistryBuilder, RegistryEntry, RegistryMeta, RegistryMetaStatic, RegistryRef, RegistrySource, RuntimeRegistry};

#[path = "compile/builtins/mod.rs"]
pub mod builtins;
#[path = "exec/context.rs"]
mod context;
#[path = "contract/def.rs"]
pub mod def;
#[path = "exec/edit_op/mod.rs"]
pub mod edit_op;
#[path = "exec/effects/mod.rs"]
mod effects;
#[path = "contract/entry.rs"]
pub mod entry;
#[path = "exec/handler.rs"]
pub mod handler;
#[path = "exec/keybindings.rs"]
mod keybindings;
#[path = "compile/link.rs"]
pub mod link;
#[path = "compile/loader.rs"]
pub mod loader;
#[path = "exec/macros.rs"]
mod macros;
#[path = "exec/pending.rs"]
mod pending;
#[path = "exec/result.rs"]
mod result;
#[path = "contract/spec.rs"]
pub mod spec;

mod domain;
#[path = "exec/editor_ctx/mod.rs"]
pub mod editor_ctx;
pub use context::{ActionArgs, ActionContext};
pub use def::{ActionDef, ActionHandler};
pub use domain::Actions;
pub use editor_ctx::{
	CursorAccess, DeferredInvocationAccess, EditAccess, EditorCapabilities, EditorContext, EditorOps, FileOpsAccess, FocusOps, HandleOutcome, JumpAccess,
	MacroAccess, ModeAccess, MotionAccess, MotionDispatchAccess, NotificationAccess, OptionAccess, PaletteAccess, SearchAccess, SelectionAccess, SplitOps,
	TextAccess, ThemeAccess, UndoAccess, ViewportAccess,
};
pub use entry::ActionEntry;
pub use handler::{ActionHandlerReg, ActionHandlerStatic};

// Re-export macros
pub use crate::action_handler;
pub use crate::core::{ActionId, LookupKey};

/// Typed handle for looking up an action by canonical ID or registry reference.
pub type ActionKey = LookupKey<ActionEntry, ActionId>;
/// Typed reference to a runtime action entry.
pub type ActionRef = RegistryRef<ActionEntry, ActionId>;
pub use builtins::{cursor_motion, selection_motion};
pub use edit_op::{CharMapKind, CursorAdjust, EditOp, EditPlan, PostEffect, PreEffect, SelectionOp, TextTransform};
pub use effects::{ActionEffects, AppEffect, DeferredInvocationRequest, EditEffect, Effect, MotionKind, MotionRequest, ScrollAmount, UiEffect, ViewEffect};
pub use keybindings::{BindingMode, KeyBindingDef};
pub use pending::PendingAction;
pub use result::{ActionResult, ScreenPosition};
pub use xeno_primitives::{Axis, Mode, ObjectSelectionKind, PendingKind, SeqDirection, SpatialDirection};

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

/// Registers compiled actions and prefixes from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_actions_spec();
	let handlers = inventory::iter::<handler::ActionHandlerReg>.into_iter().map(|r| r.0);

	let linked = link::link_actions(&spec, handlers);

	for def in linked {
		db.push_domain::<Actions>(def::ActionInput::Linked(def));
	}
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
