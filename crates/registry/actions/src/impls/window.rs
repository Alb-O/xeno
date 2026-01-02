//!
//! Split names follow Vim/Helix conventions based on the divider line orientation:
//! - `split_horizontal` (Ctrl+w s): horizontal divider → windows stacked top/bottom
//! - `split_vertical` (Ctrl+w v): vertical divider → windows side-by-side left/right

use crate::editor_ctx::HandleOutcome;
use crate::{action, result_handler, ActionResult};

action!(split_horizontal, {
	description: "Split horizontally (new buffer below)",
	bindings: r#"window "s""#,
}, |_ctx| ActionResult::SplitHorizontal);

action!(split_vertical, {
	description: "Split vertically (new buffer to right)",
	bindings: r#"window "v""#,
}, |_ctx| ActionResult::SplitVertical);

result_handler!(
	RESULT_SPLIT_HORIZONTAL_HANDLERS,
	SPLIT_HORIZONTAL_HANDLER,
	"split_horizontal",
	|result, ctx, _extend| {
		let ActionResult::SplitHorizontal = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.split_ops() {
			ops.split_horizontal();
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_SPLIT_VERTICAL_HANDLERS,
	SPLIT_VERTICAL_HANDLER,
	"split_vertical",
	|result, ctx, _extend| {
		let ActionResult::SplitVertical = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.split_ops() {
			ops.split_vertical();
		}
		HandleOutcome::Handled
	}
);

action!(toggle_terminal, {
	description: "Toggle terminal split",
	bindings: r#"normal ":""#,
}, |_ctx| ActionResult::TogglePanel("terminal"));

action!(toggle_debug_panel, {
	description: "Toggle debug panel",
	bindings: r#"normal "D""#,
}, |_ctx| ActionResult::TogglePanel("debug"));

result_handler!(
	RESULT_TOGGLE_PANEL_HANDLERS,
	TOGGLE_PANEL_HANDLER,
	"toggle_panel",
	|result, ctx, _extend| {
		let ActionResult::TogglePanel(name) = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.panel_ops() {
			ops.toggle_panel(name);
		}
		HandleOutcome::Handled
	}
);

action!(focus_left, {
	description: "Focus split to the left",
	bindings: r#"window "h""#,
}, |_ctx| ActionResult::FocusLeft);

action!(focus_down, {
	description: "Focus split below",
	bindings: r#"window "j""#,
}, |_ctx| ActionResult::FocusDown);

action!(focus_up, {
	description: "Focus split above",
	bindings: r#"window "k""#,
}, |_ctx| ActionResult::FocusUp);

action!(focus_right, {
	description: "Focus split to the right",
	bindings: r#"window "l""#,
}, |_ctx| ActionResult::FocusRight);

result_handler!(
	RESULT_FOCUS_LEFT_HANDLERS,
	FOCUS_LEFT_HANDLER,
	"focus_left",
	|result, ctx, _extend| {
		let ActionResult::FocusLeft = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.focus_ops() {
			ops.focus_left();
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_FOCUS_DOWN_HANDLERS,
	FOCUS_DOWN_HANDLER,
	"focus_down",
	|result, ctx, _extend| {
		let ActionResult::FocusDown = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.focus_ops() {
			ops.focus_down();
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_FOCUS_UP_HANDLERS,
	FOCUS_UP_HANDLER,
	"focus_up",
	|result, ctx, _extend| {
		let ActionResult::FocusUp = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.focus_ops() {
			ops.focus_up();
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_FOCUS_RIGHT_HANDLERS,
	FOCUS_RIGHT_HANDLER,
	"focus_right",
	|result, ctx, _extend| {
		let ActionResult::FocusRight = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.focus_ops() {
			ops.focus_right();
		}
		HandleOutcome::Handled
	}
);

action!(buffer_next, {
	description: "Switch to next buffer",
	bindings: r#"window "n""#,
}, |_ctx| ActionResult::BufferNext);

action!(buffer_prev, {
	description: "Switch to previous buffer",
	bindings: r#"window "p""#,
}, |_ctx| ActionResult::BufferPrev);

result_handler!(
	RESULT_BUFFER_NEXT_HANDLERS,
	BUFFER_NEXT_HANDLER,
	"buffer_next",
	|result, ctx, _extend| {
		let ActionResult::BufferNext = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.focus_ops() {
			ops.buffer_next();
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_BUFFER_PREV_HANDLERS,
	BUFFER_PREV_HANDLER,
	"buffer_prev",
	|result, ctx, _extend| {
		let ActionResult::BufferPrev = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.focus_ops() {
			ops.buffer_prev();
		}
		HandleOutcome::Handled
	}
);

action!(close_split, {
	description: "Close current split",
	bindings: r#"window "q" "c""#,
}, |_ctx| ActionResult::CloseSplit);

action!(close_other_buffers, {
	description: "Close all other buffers",
	bindings: r#"window "o""#,
}, |_ctx| ActionResult::CloseOtherBuffers);

result_handler!(
	RESULT_CLOSE_SPLIT_HANDLERS,
	CLOSE_SPLIT_HANDLER,
	"close_split",
	|result, ctx, _extend| {
		let ActionResult::CloseSplit = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.split_ops() {
			ops.close_split();
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_CLOSE_OTHER_BUFFERS_HANDLERS,
	CLOSE_OTHER_BUFFERS_HANDLER,
	"close_other_buffers",
	|result, ctx, _extend| {
		let ActionResult::CloseOtherBuffers = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.split_ops() {
			ops.close_other_buffers();
		}
		HandleOutcome::Handled
	}
);
