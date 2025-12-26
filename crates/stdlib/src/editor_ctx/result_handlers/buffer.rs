//! Buffer and split management result handlers.

use tome_manifest::actions::ActionResult;
use tome_manifest::editor_ctx::HandleOutcome;
use tome_manifest::result_handler;

use crate::NotifyWARNExt;

result_handler!(
	RESULT_SPLIT_HORIZONTAL_HANDLERS,
	HANDLE_SPLIT_HORIZONTAL,
	"split_horizontal",
	|r, ctx, _| {
		if matches!(r, ActionResult::SplitHorizontal) {
			if let Some(ops) = ctx.buffer_ops() {
				ops.split_horizontal();
			} else {
				ctx.warn("Buffer operations not available");
			}
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_SPLIT_VERTICAL_HANDLERS,
	HANDLE_SPLIT_VERTICAL,
	"split_vertical",
	|r, ctx, _| {
		if matches!(r, ActionResult::SplitVertical) {
			if let Some(ops) = ctx.buffer_ops() {
				ops.split_vertical();
			} else {
				ctx.warn("Buffer operations not available");
			}
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_BUFFER_NEXT_HANDLERS,
	HANDLE_BUFFER_NEXT,
	"buffer_next",
	|r, ctx, _| {
		if matches!(r, ActionResult::BufferNext) {
			if let Some(ops) = ctx.buffer_ops() {
				ops.buffer_next();
			} else {
				ctx.warn("Buffer operations not available");
			}
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_BUFFER_PREV_HANDLERS,
	HANDLE_BUFFER_PREV,
	"buffer_prev",
	|r, ctx, _| {
		if matches!(r, ActionResult::BufferPrev) {
			if let Some(ops) = ctx.buffer_ops() {
				ops.buffer_prev();
			} else {
				ctx.warn("Buffer operations not available");
			}
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_CLOSE_BUFFER_HANDLERS,
	HANDLE_CLOSE_BUFFER,
	"close_buffer",
	|r, ctx, _| {
		if matches!(r, ActionResult::CloseBuffer) {
			if let Some(ops) = ctx.buffer_ops() {
				ops.close_buffer();
			} else {
				ctx.warn("Buffer operations not available");
			}
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_CLOSE_OTHER_BUFFERS_HANDLERS,
	HANDLE_CLOSE_OTHER_BUFFERS,
	"close_other_buffers",
	|r, ctx, _| {
		if matches!(r, ActionResult::CloseOtherBuffers) {
			if let Some(ops) = ctx.buffer_ops() {
				ops.close_other_buffers();
			} else {
				ctx.warn("Buffer operations not available");
			}
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_FOCUS_LEFT_HANDLERS,
	HANDLE_FOCUS_LEFT,
	"focus_left",
	|r, ctx, _| {
		if matches!(r, ActionResult::FocusLeft) {
			if let Some(ops) = ctx.buffer_ops() {
				ops.focus_left();
			} else {
				ctx.warn("Buffer operations not available");
			}
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_FOCUS_RIGHT_HANDLERS,
	HANDLE_FOCUS_RIGHT,
	"focus_right",
	|r, ctx, _| {
		if matches!(r, ActionResult::FocusRight) {
			if let Some(ops) = ctx.buffer_ops() {
				ops.focus_right();
			} else {
				ctx.warn("Buffer operations not available");
			}
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_FOCUS_UP_HANDLERS,
	HANDLE_FOCUS_UP,
	"focus_up",
	|r, ctx, _| {
		if matches!(r, ActionResult::FocusUp) {
			if let Some(ops) = ctx.buffer_ops() {
				ops.focus_up();
			} else {
				ctx.warn("Buffer operations not available");
			}
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_FOCUS_DOWN_HANDLERS,
	HANDLE_FOCUS_DOWN,
	"focus_down",
	|r, ctx, _| {
		if matches!(r, ActionResult::FocusDown) {
			if let Some(ops) = ctx.buffer_ops() {
				ops.focus_down();
			} else {
				ctx.warn("Buffer operations not available");
			}
		}
		HandleOutcome::Handled
	}
);
