//! Insert mode text entry actions.

use xeno_primitives::range::Range;
use xeno_primitives::{Mode, Selection};
use xeno_registry_motions::keys as motions;
use xeno_registry_motions::MotionKey;

use crate::{ActionContext, ActionEffects, ActionResult, AppEffect, Effect, action, edit_op};

/// Applies a typed motion to all cursors before entering insert mode.
pub fn insert_with_motion(ctx: &ActionContext, motion: MotionKey) -> ActionResult {
	let motion_def = motion.def();
	let mut new_selection = ctx.selection.clone();
	new_selection.transform_mut(|range| {
		*range = (motion_def.handler)(ctx.text, *range, 1, false);
	});

	ActionResult::Effects(
		ActionEffects::motion(new_selection).with(Effect::App(AppEffect::SetMode(Mode::Insert))),
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
	|ctx| insert_with_motion(ctx, motions::line_start));

action!(insert_line_end, { description: "Insert at end of line", bindings: r#"normal "A""# },
	|ctx| insert_with_motion(ctx, motions::line_end));

action!(insert_after, { description: "Insert after cursor", bindings: r#"normal "a""# }, |ctx| {
	let max_pos = ctx.text.len_chars();
	let ranges: Vec<_> = ctx.selection.ranges().iter()
		.map(|r| Range::new(r.min(), (r.max() + 1).min(max_pos)))
		.collect();
	let sel = Selection::from_vec(ranges, ctx.selection.primary_index());
	ActionResult::Effects(ActionEffects::motion(sel).with(Effect::App(AppEffect::SetMode(Mode::Insert))))
});

action!(insert_newline, {
	description: "Insert newline with indentation",
	bindings: r#"insert "enter""#,
}, |_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::insert_newline())));
