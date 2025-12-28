//! Action result handler registry.
//!
//! Handlers are registered at compile-time and dispatched based on the
//! ActionResult variant they handle, with per-variant slices to avoid
//! runtime scanning.

use linkme::distributed_slice;

use super::EditorContext;
use super::capabilities::MessageAccess;
use crate::actions::ActionResult;

/// Outcome of handling an action result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleOutcome {
	/// Result was handled, continue running.
	Handled,
	/// Result was handled, editor should quit.
	Quit,
	/// This handler explicitly declined the result (try next handler in slice).
	NotHandled,
}

/// A handler for a specific ActionResult variant.
pub struct ResultHandler {
	/// Name for debugging/logging.
	pub name: &'static str,
	/// Handle the result, returning the outcome.
	pub handle: fn(&ActionResult, &mut EditorContext, bool) -> HandleOutcome,
}

impl std::fmt::Debug for ResultHandler {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ResultHandler")
			.field("name", &self.name)
			.finish()
	}
}

macro_rules! result_slices {
    ($($slice:ident),+ $(,)?) => {
        $(#[distributed_slice]
        pub static $slice: [ResultHandler];)+
    };
}

result_slices!(
	RESULT_OK_HANDLERS,
	RESULT_MODE_CHANGE_HANDLERS,
	RESULT_CURSOR_MOVE_HANDLERS,
	RESULT_MOTION_HANDLERS,
	RESULT_INSERT_WITH_MOTION_HANDLERS,
	RESULT_EDIT_HANDLERS,
	RESULT_QUIT_HANDLERS,
	RESULT_ERROR_HANDLERS,
	RESULT_PENDING_HANDLERS,
	RESULT_SEARCH_NEXT_HANDLERS,
	RESULT_SEARCH_PREV_HANDLERS,
	RESULT_USE_SELECTION_SEARCH_HANDLERS,
	RESULT_FORCE_REDRAW_HANDLERS,
	// Unimplemented stubs (have action! but no real handler yet)
	RESULT_ALIGN_HANDLERS,
	RESULT_COPY_INDENT_HANDLERS,
	RESULT_TABS_TO_SPACES_HANDLERS,
	RESULT_SPACES_TO_TABS_HANDLERS,
	RESULT_TRIM_SELECTIONS_HANDLERS,
	// Buffer/split management
	RESULT_SPLIT_HORIZONTAL_HANDLERS,
	RESULT_SPLIT_VERTICAL_HANDLERS,
	RESULT_SPLIT_TERMINAL_HORIZONTAL_HANDLERS,
	RESULT_SPLIT_TERMINAL_VERTICAL_HANDLERS,
	RESULT_BUFFER_NEXT_HANDLERS,
	RESULT_BUFFER_PREV_HANDLERS,
	RESULT_CLOSE_BUFFER_HANDLERS,
	RESULT_CLOSE_OTHER_BUFFERS_HANDLERS,
	RESULT_FOCUS_LEFT_HANDLERS,
	RESULT_FOCUS_RIGHT_HANDLERS,
	RESULT_FOCUS_UP_HANDLERS,
	RESULT_FOCUS_DOWN_HANDLERS,
);

fn run_handlers(
	handlers: &[ResultHandler],
	result: &ActionResult,
	ctx: &mut EditorContext,
	extend: bool,
) -> bool {
	for handler in handlers {
		match (handler.handle)(result, ctx, extend) {
			HandleOutcome::Handled => return false,
			HandleOutcome::Quit => return true,
			HandleOutcome::NotHandled => continue,
		}
	}
	ctx.notify(
		"info",
		&format!(
			"Unhandled action result: {:?}",
			std::mem::discriminant(result)
		),
	);
	false
}

/// Find and execute the handler for a result.
/// Returns true if the editor should quit.
pub fn dispatch_result(result: &ActionResult, ctx: &mut EditorContext, extend: bool) -> bool {
	match result {
		ActionResult::Ok => run_handlers(&RESULT_OK_HANDLERS, result, ctx, extend),
		ActionResult::ModeChange(_) => {
			run_handlers(&RESULT_MODE_CHANGE_HANDLERS, result, ctx, extend)
		}
		ActionResult::CursorMove(_) => {
			run_handlers(&RESULT_CURSOR_MOVE_HANDLERS, result, ctx, extend)
		}
		ActionResult::Motion(_) => run_handlers(&RESULT_MOTION_HANDLERS, result, ctx, extend),
		ActionResult::InsertWithMotion(_) => {
			run_handlers(&RESULT_INSERT_WITH_MOTION_HANDLERS, result, ctx, extend)
		}
		ActionResult::Edit(_) => run_handlers(&RESULT_EDIT_HANDLERS, result, ctx, extend),
		ActionResult::Quit | ActionResult::ForceQuit => {
			run_handlers(&RESULT_QUIT_HANDLERS, result, ctx, extend)
		}
		ActionResult::Error(_) => run_handlers(&RESULT_ERROR_HANDLERS, result, ctx, extend),
		ActionResult::Pending(_) => run_handlers(&RESULT_PENDING_HANDLERS, result, ctx, extend),
		ActionResult::SearchNext { .. } => {
			run_handlers(&RESULT_SEARCH_NEXT_HANDLERS, result, ctx, extend)
		}
		ActionResult::SearchPrev { .. } => {
			run_handlers(&RESULT_SEARCH_PREV_HANDLERS, result, ctx, extend)
		}
		ActionResult::UseSelectionAsSearch => {
			run_handlers(&RESULT_USE_SELECTION_SEARCH_HANDLERS, result, ctx, extend)
		}
		ActionResult::ForceRedraw => {
			run_handlers(&RESULT_FORCE_REDRAW_HANDLERS, result, ctx, extend)
		}
		ActionResult::Align => run_handlers(&RESULT_ALIGN_HANDLERS, result, ctx, extend),
		ActionResult::CopyIndent => run_handlers(&RESULT_COPY_INDENT_HANDLERS, result, ctx, extend),
		ActionResult::TabsToSpaces => {
			run_handlers(&RESULT_TABS_TO_SPACES_HANDLERS, result, ctx, extend)
		}
		ActionResult::SpacesToTabs => {
			run_handlers(&RESULT_SPACES_TO_TABS_HANDLERS, result, ctx, extend)
		}
		ActionResult::TrimSelections => {
			run_handlers(&RESULT_TRIM_SELECTIONS_HANDLERS, result, ctx, extend)
		}
		// Buffer/split management
		ActionResult::SplitHorizontal => {
			run_handlers(&RESULT_SPLIT_HORIZONTAL_HANDLERS, result, ctx, extend)
		}
		ActionResult::SplitVertical => {
			run_handlers(&RESULT_SPLIT_VERTICAL_HANDLERS, result, ctx, extend)
		}
		ActionResult::SplitTerminalHorizontal => run_handlers(
			&RESULT_SPLIT_TERMINAL_HORIZONTAL_HANDLERS,
			result,
			ctx,
			extend,
		),
		ActionResult::SplitTerminalVertical => run_handlers(
			&RESULT_SPLIT_TERMINAL_VERTICAL_HANDLERS,
			result,
			ctx,
			extend,
		),
		ActionResult::BufferNext => run_handlers(&RESULT_BUFFER_NEXT_HANDLERS, result, ctx, extend),
		ActionResult::BufferPrev => run_handlers(&RESULT_BUFFER_PREV_HANDLERS, result, ctx, extend),
		ActionResult::CloseBuffer => {
			run_handlers(&RESULT_CLOSE_BUFFER_HANDLERS, result, ctx, extend)
		}
		ActionResult::CloseOtherBuffers => {
			run_handlers(&RESULT_CLOSE_OTHER_BUFFERS_HANDLERS, result, ctx, extend)
		}
		ActionResult::FocusLeft => run_handlers(&RESULT_FOCUS_LEFT_HANDLERS, result, ctx, extend),
		ActionResult::FocusRight => run_handlers(&RESULT_FOCUS_RIGHT_HANDLERS, result, ctx, extend),
		ActionResult::FocusUp => run_handlers(&RESULT_FOCUS_UP_HANDLERS, result, ctx, extend),
		ActionResult::FocusDown => run_handlers(&RESULT_FOCUS_DOWN_HANDLERS, result, ctx, extend),
	}
}

/// Macro to simplify handler registration per variant.
#[macro_export]
macro_rules! result_handler {
	($slice:ident, $static_name:ident, $name:literal, $body:expr) => {
		#[::linkme::distributed_slice($crate::editor_ctx::$slice)]
		static $static_name: $crate::editor_ctx::ResultHandler =
			$crate::editor_ctx::ResultHandler {
				name: $name,
				handle: $body,
			};
	};
}

// Integration tests that require tome-stdlib are in tests/registry.rs
