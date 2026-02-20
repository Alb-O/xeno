use xeno_primitives::range::Range;
use xeno_primitives::{Mode, MotionId, Selection, motion_ids};

use crate::actions::{ActionEffects, ActionResult, AppEffect, Effect, MotionKind, MotionRequest, ViewEffect, action_handler, edit_op};

/// Emits a motion request followed by a mode change to insert mode.
fn insert_with_motion(_ctx: &crate::actions::ActionContext, id: MotionId) -> ActionResult {
	ActionResult::Effects(
		ActionEffects::from_effect(
			ViewEffect::Motion(MotionRequest {
				id,
				count: 1,
				extend: false,
				kind: MotionKind::Cursor,
			})
			.into(),
		)
		.with(Effect::App(AppEffect::SetMode(Mode::Insert))),
	)
}

action_handler!(insert_mode, |ctx| {
	let ranges: Vec<_> = ctx.selection.ranges().iter().map(|r| Range::new(r.max(), r.min())).collect();
	let sel = Selection::from_vec(ranges, ctx.selection.primary_index());
	ActionResult::Effects(ActionEffects::selection(sel).with(Effect::App(AppEffect::SetMode(Mode::Insert))))
});

action_handler!(insert_line_start, |ctx| insert_with_motion(ctx, motion_ids::LINE_START));
action_handler!(insert_line_end, |ctx| insert_with_motion(ctx, motion_ids::LINE_END));

action_handler!(insert_after, |ctx| {
	let max_pos = ctx.text.len_chars();
	let ranges: Vec<_> = ctx.selection.ranges().iter().map(|r| Range::point((r.max() + 1).min(max_pos))).collect();
	let sel = Selection::from_vec(ranges, ctx.selection.primary_index());
	ActionResult::Effects(ActionEffects::selection(sel).with(Effect::App(AppEffect::SetMode(Mode::Insert))))
});

action_handler!(insert_newline, |_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::insert_newline())));
