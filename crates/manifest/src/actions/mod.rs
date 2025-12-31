//! Action definitions and execution context.
//!
//! This module contains the types for defining and executing actions:
//!
//! - [`ActionResult`] - What an action returns to describe editor changes
//! - [`EditAction`] - Text modification operations
//! - [`ActionContext`] - Read-only context passed to action handlers
//! - [`ActionDef`] - Compile-time action registration
//! - [`PendingAction`] - State for multi-key sequences
//!
//! Motion helpers for applying named motions:
//! - [`cursor_motion`] - Move cursor using a motion
//! - [`selection_motion`] - Create selection using a motion
//! - [`insert_with_motion`] - Enter insert mode after motion

mod context;
mod definition;
mod edit;
mod motion;
mod pending;
mod result;

pub use context::{ActionArgs, ActionContext};
pub use definition::{ActionDef, ActionHandler};
pub use edit::{EditAction, ScrollAmount, ScrollDir, VisualDirection};
pub use motion::{cursor_motion, insert_with_motion, selection_motion};
pub use pending::{ObjectSelectionKind, PendingAction, PendingKind};
pub use result::{
	ActionMode, ActionResult, RESULT_BUFFER_NEXT_HANDLERS, RESULT_BUFFER_PREV_HANDLERS,
	RESULT_CLOSE_OTHER_BUFFERS_HANDLERS, RESULT_CLOSE_SPLIT_HANDLERS, RESULT_CURSOR_MOVE_HANDLERS,
	RESULT_EDIT_HANDLERS, RESULT_ERROR_HANDLERS, RESULT_EXTENSION_HANDLERS,
	RESULT_FOCUS_DOWN_HANDLERS, RESULT_FOCUS_LEFT_HANDLERS, RESULT_FOCUS_RIGHT_HANDLERS,
	RESULT_FOCUS_UP_HANDLERS, RESULT_FORCE_REDRAW_HANDLERS, RESULT_INSERT_WITH_MOTION_HANDLERS,
	RESULT_MODE_CHANGE_HANDLERS, RESULT_MOTION_HANDLERS, RESULT_OK_HANDLERS,
	RESULT_PENDING_HANDLERS, RESULT_QUIT_HANDLERS, RESULT_SEARCH_NEXT_HANDLERS,
	RESULT_SEARCH_PREV_HANDLERS, RESULT_SPLIT_HORIZONTAL_HANDLERS,
	RESULT_SPLIT_VERTICAL_HANDLERS, RESULT_TOGGLE_PANEL_HANDLERS,
	RESULT_USE_SELECTION_SEARCH_HANDLERS, dispatch_result,
};
