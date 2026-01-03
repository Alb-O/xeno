//! Split and window management actions.
//!
//! Split names follow Vim/Helix conventions based on the divider line orientation:
//! - `split_horizontal`: horizontal divider → windows stacked top/bottom
//! - `split_vertical`: vertical divider → windows side-by-side left/right
//!
//! Bindings use hierarchical key sequences under `ctrl-w`:
//! - `s h/v` - Split horizontal/vertical
//! - `f h/j/k/l` - Focus directions
//! - `b n/p` - Buffer navigation
//! - `c c/o` - Close current/others

use evildoer_registry_panels::keys as panels;

use crate::editor_ctx::HandleOutcome;
use crate::{ActionResult, action, result_handler};

action!(split_horizontal, {
	description: "Split horizontal",
	short_desc: "Horizontal",
	bindings: r#"normal "ctrl-w s h""#,
}, |_ctx| ActionResult::SplitHorizontal);

action!(split_vertical, {
	description: "Split vertical",
	short_desc: "Vertical",
	bindings: r#"normal "ctrl-w s v""#,
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
}, |_ctx| ActionResult::TogglePanel(panels::terminal));

action!(toggle_debug_panel, {
	description: "Toggle debug panel",
	bindings: r#"normal "D""#,
}, |_ctx| ActionResult::TogglePanel(panels::debug));

result_handler!(
	RESULT_TOGGLE_PANEL_HANDLERS,
	TOGGLE_PANEL_HANDLER,
	"toggle_panel",
	|result, ctx, _extend| {
		let ActionResult::TogglePanel(panel) = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.panel_ops() {
			ops.toggle_panel(panel.name());
		}
		HandleOutcome::Handled
	}
);

action!(focus_left, {
	description: "Focus left",
	short_desc: "Left",
	bindings: r#"normal "ctrl-w f h""#,
}, |_ctx| ActionResult::FocusLeft);

action!(focus_down, {
	description: "Focus down",
	short_desc: "Down",
	bindings: r#"normal "ctrl-w f j""#,
}, |_ctx| ActionResult::FocusDown);

action!(focus_up, {
	description: "Focus up",
	short_desc: "Up",
	bindings: r#"normal "ctrl-w f k""#,
}, |_ctx| ActionResult::FocusUp);

action!(focus_right, {
	description: "Focus right",
	short_desc: "Right",
	bindings: r#"normal "ctrl-w f l""#,
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
	description: "Next buffer",
	short_desc: "Next",
	bindings: r#"normal "ctrl-w b n""#,
}, |_ctx| ActionResult::BufferNext);

action!(buffer_prev, {
	description: "Previous buffer",
	short_desc: "Previous",
	bindings: r#"normal "ctrl-w b p""#,
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
	short_desc: "Current",
	bindings: r#"normal "ctrl-w c c""#,
}, |_ctx| ActionResult::CloseSplit);

action!(close_other_buffers, {
	description: "Close other buffers",
	short_desc: "Others",
	bindings: r#"normal "ctrl-w c o""#,
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
