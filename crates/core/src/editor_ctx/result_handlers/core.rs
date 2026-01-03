//! Core result handlers: Ok, CursorMove, Motion, Edit, Quit, Error.

use xeno_base::Selection;
use xeno_base::range::Range;
use xeno_registry::actions::ScreenPosition;
use xeno_registry::{
	ActionResult, HandleOutcome, HookContext, HookEventData, Mode, emit_sync as emit_hook_sync,
	notification_keys as keys, result_handler,
};

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
			if let Some((line, col)) = ctx.cursor_line_col() {
				emit_hook_sync(&HookContext::new(
					HookEventData::CursorMove { line, col },
					None,
				));
			}
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
			let primary = sel.primary();
			if let Some((line, col)) = ctx.cursor_line_col() {
				emit_hook_sync(&HookContext::new(
					HookEventData::CursorMove { line, col },
					None,
				));
			}
			emit_hook_sync(&HookContext::new(
				HookEventData::SelectionChange {
					anchor: primary.anchor,
					head: primary.head,
				},
				None,
			));
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_SCREEN_MOTION_HANDLERS,
	HANDLE_SCREEN_MOTION,
	"screen_motion",
	|r, ctx, extend| {
		let ActionResult::ScreenMotion { position, count } = r else {
			return HandleOutcome::NotHandled;
		};
		let Some(viewport) = ctx.viewport() else {
			ctx.emit(keys::viewport_unavailable);
			return HandleOutcome::Handled;
		};
		let height = viewport.viewport_height();
		if height == 0 {
			ctx.emit(keys::viewport_height_unavailable);
			return HandleOutcome::Handled;
		}
		let count = (*count).max(1);
		let mut row = match position {
			ScreenPosition::Top => count.saturating_sub(1),
			ScreenPosition::Middle => height / 2 + count.saturating_sub(1),
			ScreenPosition::Bottom => height.saturating_sub(count),
		};
		if row >= height {
			row = height.saturating_sub(1);
		}

		let Some(target) = viewport.viewport_row_to_doc_position(row) else {
			ctx.emit(keys::screen_motion_unavailable);
			return HandleOutcome::Handled;
		};

		let selection = ctx.selection();
		let primary_index = selection.primary_index();
		let new_ranges: Vec<Range> = selection
			.ranges()
			.iter()
			.map(|range| {
				if extend {
					Range::new(range.anchor, target)
				} else {
					Range::point(target)
				}
			})
			.collect();
		let new_selection = Selection::from_vec(new_ranges, primary_index);
		ctx.set_cursor(new_selection.primary().head);
		ctx.set_selection(new_selection.clone());

		let primary = new_selection.primary();
		if let Some((line, col)) = ctx.cursor_line_col() {
			emit_hook_sync(&HookContext::new(
				HookEventData::CursorMove { line, col },
				None,
			));
		}
		emit_hook_sync(&HookContext::new(
			HookEventData::SelectionChange {
				anchor: primary.anchor,
				head: primary.head,
			},
			None,
		));
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
			let primary = sel.primary();
			if let Some((line, col)) = ctx.cursor_line_col() {
				emit_hook_sync(&HookContext::new(
					HookEventData::CursorMove { line, col },
					None,
				));
			}
			emit_hook_sync(&HookContext::new(
				HookEventData::SelectionChange {
					anchor: primary.anchor,
					head: primary.head,
				},
				None,
			));
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
		ctx.emit(keys::action_error::call(msg));
	}
	HandleOutcome::Handled
});

result_handler!(
	RESULT_PENDING_HANDLERS,
	HANDLE_PENDING,
	"pending",
	|r, ctx, _| {
		if let ActionResult::Pending(pending) = r {
			ctx.emit(keys::pending_prompt::call(&pending.prompt));
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

result_handler!(
	RESULT_COMMAND_HANDLERS,
	HANDLE_COMMAND,
	"command",
	|r, ctx, _| {
		if let ActionResult::Command { name, args } = r
			&& let Some(queue) = ctx.command_queue()
		{
			queue.queue_command(name, args.clone());
		}
		HandleOutcome::Handled
	}
);
