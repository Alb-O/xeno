//! Core result handlers: Ok, CursorMove, Motion, Edit, Quit, Error.

use crate::ext::actions::ActionResult;
use crate::ext::editor_ctx::HandleOutcome;
use crate::{Mode, result_handler};

result_handler!(RESULT_OK_HANDLERS, HANDLE_OK, "ok", |_, _, _| {
	HandleOutcome::Handled
});

result_handler!(
	RESULT_CURSOR_MOVE_HANDLERS,
	HANDLE_CURSOR_MOVE,
	"cursor_move",
	|r, ctx, _| {
		if let ActionResult::CursorMove(pos) = r {
			ctx.set_cursor(*pos);
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_MOTION_HANDLERS,
	HANDLE_MOTION,
	"motion",
	|r, ctx, _| {
		if let ActionResult::Motion(sel) = r {
			ctx.set_cursor(sel.primary().head);
			ctx.set_selection(sel.clone());
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_INSERT_WITH_MOTION_HANDLERS,
	HANDLE_INSERT_WITH_MOTION,
	"insert_with_motion",
	|r, ctx, _| {
		if let ActionResult::InsertWithMotion(sel) = r {
			ctx.set_cursor(sel.primary().head);
			ctx.set_selection(sel.clone());
			ctx.set_mode(Mode::Insert);
		}
		HandleOutcome::Handled
	}
);

result_handler!(RESULT_QUIT_HANDLERS, HANDLE_QUIT, "quit", |_, _, _| {
	HandleOutcome::Quit
});

result_handler!(RESULT_ERROR_HANDLERS, HANDLE_ERROR, "error", |r, ctx, _| {
	if let ActionResult::Error(msg) = r {
		ctx.message(msg);
	}
	HandleOutcome::Handled
});

result_handler!(
	RESULT_PENDING_HANDLERS,
	HANDLE_PENDING,
	"pending",
	|r, ctx, _| {
		if let ActionResult::Pending(pending) = r {
			ctx.message(&pending.prompt);
			ctx.set_mode(Mode::PendingAction(pending.kind));
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_FORCE_REDRAW_HANDLERS,
	HANDLE_FORCE_REDRAW,
	"force_redraw",
	|_, _, _| HandleOutcome::Handled
);
