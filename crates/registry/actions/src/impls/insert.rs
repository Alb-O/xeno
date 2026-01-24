//! Insert mode text entry actions.

use xeno_primitives::range::Range;
use xeno_primitives::{Mode, MotionId, Selection, motion_ids};

use crate::{
	ActionContext, ActionEffects, ActionResult, AppEffect, Effect, MotionKind, MotionRequest,
	ViewEffect, action, edit_op,
};

/// Emits a motion request followed by a mode change to insert mode.
///
/// The executor will apply the motion first, then switch to insert mode.
pub fn insert_with_motion(_ctx: &ActionContext, id: MotionId) -> ActionResult {
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

action!(insert_mode, { description: "Switch to insert mode", bindings: r#"normal "i""# }, |ctx| {
	let ranges: Vec<_> = ctx.selection.ranges().iter()
		.map(|r| Range::new(r.max(), r.min()))
		.collect();
	let sel = Selection::from_vec(ranges, ctx.selection.primary_index());
	ActionResult::Effects(ActionEffects::motion(sel).with(Effect::App(AppEffect::SetMode(Mode::Insert))))
});

action!(insert_line_start, { description: "Insert at start of line", bindings: r#"normal "I""# },
	|ctx| insert_with_motion(ctx, motion_ids::LINE_START));

action!(insert_line_end, { description: "Insert at end of line", bindings: r#"normal "A""# },
	|ctx| insert_with_motion(ctx, motion_ids::LINE_END));

action!(insert_after, { description: "Insert after cursor", bindings: r#"normal "a""# }, |ctx| {
	let max_pos = ctx.text.len_chars();
	let ranges: Vec<_> = ctx.selection.ranges().iter()
		.map(|r| Range::point((r.max() + 1).min(max_pos)))
		.collect();
	let sel = Selection::from_vec(ranges, ctx.selection.primary_index());
	ActionResult::Effects(ActionEffects::motion(sel).with(Effect::App(AppEffect::SetMode(Mode::Insert))))
});

action!(insert_newline, {
	description: "Insert newline with indentation",
	bindings: r#"insert "enter""#,
}, |_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::insert_newline())));
