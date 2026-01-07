use xeno_base::range::Range;
use xeno_base::{Mode, Selection};
use xeno_registry_motions::keys as motions;

use crate::{ActionEffects, ActionResult, Effect, action, insert_with_motion};

action!(insert_mode, { description: "Switch to insert mode", bindings: r#"normal "i""# }, |ctx| {
	let ranges: Vec<_> = ctx.selection.ranges().iter()
		.map(|r| Range::new(r.max(), r.min()))
		.collect();
	let sel = Selection::from_vec(ranges, ctx.selection.primary_index());
	ActionResult::Effects(ActionEffects::motion(sel).with(Effect::SetMode(Mode::Insert)))
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
	ActionResult::Effects(ActionEffects::motion(sel).with(Effect::SetMode(Mode::Insert)))
});
