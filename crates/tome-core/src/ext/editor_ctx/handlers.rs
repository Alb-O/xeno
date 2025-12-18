//! Action result handler registry.
//!
//! Handlers are registered at compile-time and dispatched based on the
//! ActionResult variant they handle, with per-variant slices to avoid
//! runtime scanning.

use linkme::distributed_slice;

use super::EditorContext;
use crate::ext::actions::ActionResult;

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
	RESULT_SPLIT_LINES_HANDLERS,
	RESULT_JUMP_FORWARD_HANDLERS,
	RESULT_JUMP_BACKWARD_HANDLERS,
	RESULT_SAVE_JUMP_HANDLERS,
	RESULT_RECORD_MACRO_HANDLERS,
	RESULT_PLAY_MACRO_HANDLERS,
	RESULT_SAVE_SELECTIONS_HANDLERS,
	RESULT_RESTORE_SELECTIONS_HANDLERS,
	RESULT_FORCE_REDRAW_HANDLERS,
	RESULT_REPEAT_LAST_INSERT_HANDLERS,
	RESULT_REPEAT_LAST_OBJECT_HANDLERS,
	RESULT_DUPLICATE_SELECTIONS_DOWN_HANDLERS,
	RESULT_DUPLICATE_SELECTIONS_UP_HANDLERS,
	RESULT_MERGE_SELECTIONS_HANDLERS,
	RESULT_ALIGN_HANDLERS,
	RESULT_COPY_INDENT_HANDLERS,
	RESULT_TABS_TO_SPACES_HANDLERS,
	RESULT_SPACES_TO_TABS_HANDLERS,
	RESULT_TRIM_SELECTIONS_HANDLERS,
	RESULT_OPEN_SCRATCH_HANDLERS,
	RESULT_CLOSE_SCRATCH_HANDLERS,
	RESULT_TOGGLE_SCRATCH_HANDLERS,
	RESULT_EXECUTE_SCRATCH_HANDLERS,
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
	ctx.message(&format!(
		"Unhandled action result: {:?}",
		std::mem::discriminant(result)
	));
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
		ActionResult::SplitLines => run_handlers(&RESULT_SPLIT_LINES_HANDLERS, result, ctx, extend),
		ActionResult::JumpForward => {
			run_handlers(&RESULT_JUMP_FORWARD_HANDLERS, result, ctx, extend)
		}
		ActionResult::JumpBackward => {
			run_handlers(&RESULT_JUMP_BACKWARD_HANDLERS, result, ctx, extend)
		}
		ActionResult::SaveJump => run_handlers(&RESULT_SAVE_JUMP_HANDLERS, result, ctx, extend),
		ActionResult::RecordMacro => {
			run_handlers(&RESULT_RECORD_MACRO_HANDLERS, result, ctx, extend)
		}
		ActionResult::PlayMacro => run_handlers(&RESULT_PLAY_MACRO_HANDLERS, result, ctx, extend),
		ActionResult::SaveSelections => {
			run_handlers(&RESULT_SAVE_SELECTIONS_HANDLERS, result, ctx, extend)
		}
		ActionResult::RestoreSelections => {
			run_handlers(&RESULT_RESTORE_SELECTIONS_HANDLERS, result, ctx, extend)
		}
		ActionResult::ForceRedraw => {
			run_handlers(&RESULT_FORCE_REDRAW_HANDLERS, result, ctx, extend)
		}
		ActionResult::RepeatLastInsert => {
			run_handlers(&RESULT_REPEAT_LAST_INSERT_HANDLERS, result, ctx, extend)
		}
		ActionResult::RepeatLastObject => {
			run_handlers(&RESULT_REPEAT_LAST_OBJECT_HANDLERS, result, ctx, extend)
		}
		ActionResult::DuplicateSelectionsDown => run_handlers(
			&RESULT_DUPLICATE_SELECTIONS_DOWN_HANDLERS,
			result,
			ctx,
			extend,
		),
		ActionResult::DuplicateSelectionsUp => run_handlers(
			&RESULT_DUPLICATE_SELECTIONS_UP_HANDLERS,
			result,
			ctx,
			extend,
		),
		ActionResult::MergeSelections => {
			run_handlers(&RESULT_MERGE_SELECTIONS_HANDLERS, result, ctx, extend)
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
		ActionResult::OpenScratch { .. } => {
			run_handlers(&RESULT_OPEN_SCRATCH_HANDLERS, result, ctx, extend)
		}
		ActionResult::CloseScratch => {
			run_handlers(&RESULT_CLOSE_SCRATCH_HANDLERS, result, ctx, extend)
		}
		ActionResult::ToggleScratch => {
			run_handlers(&RESULT_TOGGLE_SCRATCH_HANDLERS, result, ctx, extend)
		}
		ActionResult::ExecuteScratch => {
			run_handlers(&RESULT_EXECUTE_SCRATCH_HANDLERS, result, ctx, extend)
		}
	}
}

/// Macro to simplify handler registration per variant.
#[macro_export]
macro_rules! result_handler {
	($slice:ident, $static_name:ident, $name:literal, $body:expr) => {
		#[::linkme::distributed_slice($crate::ext::editor_ctx::$slice)]
		static $static_name: $crate::ext::editor_ctx::ResultHandler =
			$crate::ext::editor_ctx::ResultHandler {
				name: $name,
				handle: $body,
			};
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_handlers_registered() {
		// Ensure common variants have at least one handler registered.
		assert!(!RESULT_OK_HANDLERS.is_empty());
		assert!(!RESULT_QUIT_HANDLERS.is_empty());
		assert!(!RESULT_ERROR_HANDLERS.is_empty());
	}

	#[test]
	fn test_handler_coverage_counts() {
		let total = RESULT_OK_HANDLERS.len()
			+ RESULT_MODE_CHANGE_HANDLERS.len()
			+ RESULT_CURSOR_MOVE_HANDLERS.len()
			+ RESULT_MOTION_HANDLERS.len()
			+ RESULT_INSERT_WITH_MOTION_HANDLERS.len()
			+ RESULT_EDIT_HANDLERS.len()
			+ RESULT_QUIT_HANDLERS.len()
			+ RESULT_ERROR_HANDLERS.len();
		assert!(
			total >= 8,
			"expected handlers registered for major variants"
		);
	}
}
