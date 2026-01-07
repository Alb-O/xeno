//! Split and window management actions.
//!
//! Split names follow Vim/Helix conventions based on the divider line orientation:
//! - `split_horizontal`: horizontal divider → windows stacked top/bottom
//! - `split_vertical`: vertical divider → windows side-by-side left/right
//!
//! Bindings use hierarchical key sequences under `ctrl-w`:
//! - `s h/v` - Split horizontal/vertical
//! - `f h/j/k/l` - Focus directions
//! - `f n/p` - Buffer next/previous
//! - `c c/o` - Close current/others

use xeno_base::direction::{Axis, SeqDirection, SpatialDirection};

use crate::editor_ctx::HandleOutcome;
use crate::{ActionResult, action, result_handler};

action!(split_horizontal, {
	description: "Split horizontal",
	short_desc: "Horizontal",
	bindings: r#"normal "ctrl-w s h""#,
}, |_ctx| ActionResult::Split(Axis::Horizontal));

action!(split_vertical, {
	description: "Split vertical",
	short_desc: "Vertical",
	bindings: r#"normal "ctrl-w s v""#,
}, |_ctx| ActionResult::Split(Axis::Vertical));

result_handler!(
	RESULT_SPLIT_HANDLERS,
	SPLIT_HANDLER,
	"split",
	|result, ctx, _extend| {
		let ActionResult::Split(axis) = result else {
			return HandleOutcome::NotHandled;
		};
		if let Some(ops) = ctx.split_ops() {
			ops.split(*axis);
		}
		HandleOutcome::Handled
	}
);

action!(focus_left, {
	description: "Focus left",
	short_desc: "Left",
	bindings: r#"normal "ctrl-w f h""#,
}, |_ctx| ActionResult::Focus(SpatialDirection::Left));

action!(focus_down, {
	description: "Focus down",
	short_desc: "Down",
	bindings: r#"normal "ctrl-w f j""#,
}, |_ctx| ActionResult::Focus(SpatialDirection::Down));

action!(focus_up, {
	description: "Focus up",
	short_desc: "Up",
	bindings: r#"normal "ctrl-w f k""#,
}, |_ctx| ActionResult::Focus(SpatialDirection::Up));

action!(focus_right, {
	description: "Focus right",
	short_desc: "Right",
	bindings: r#"normal "ctrl-w f l""#,
}, |_ctx| ActionResult::Focus(SpatialDirection::Right));

result_handler!(
	RESULT_FOCUS_HANDLERS,
	FOCUS_HANDLER,
	"focus",
	|result, ctx, _extend| {
		let ActionResult::Focus(dir) = result else {
			return HandleOutcome::NotHandled;
		};
		if let Some(ops) = ctx.focus_ops() {
			ops.focus(*dir);
		}
		HandleOutcome::Handled
	}
);

action!(buffer_next, {
	description: "Next buffer",
	short_desc: "Next",
	bindings: r#"normal "ctrl-w f n""#,
}, |_ctx| ActionResult::BufferSwitch(SeqDirection::Next));

action!(buffer_prev, {
	description: "Previous buffer",
	short_desc: "Previous",
	bindings: r#"normal "ctrl-w f p""#,
}, |_ctx| ActionResult::BufferSwitch(SeqDirection::Prev));

result_handler!(
	RESULT_BUFFER_SWITCH_HANDLERS,
	BUFFER_SWITCH_HANDLER,
	"buffer_switch",
	|result, ctx, _extend| {
		let ActionResult::BufferSwitch(dir) = result else {
			return HandleOutcome::NotHandled;
		};
		if let Some(ops) = ctx.focus_ops() {
			ops.buffer_switch(*dir);
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
