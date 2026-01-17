//! Motion-based cursor movement actions.
//!
//! This module provides actions that wrap motion primitives from `xeno-registry-motions`
//! and the helper functions for applying motions to selections.

use tracing::trace;
use xeno_primitives::Selection;
use xeno_primitives::range::Range;
use xeno_registry_motions::{MotionKey, keys as motions};

use crate::{ActionContext, ActionEffects, ActionResult, ScreenPosition, action};

/// Applies a typed motion as a cursor movement.
///
/// Uses the provided motion definition to move each cursor in the selection.
/// When `ctx.extend` is false, collapses selections to points at the new head
/// positions.
pub fn cursor_motion(ctx: &ActionContext, motion: MotionKey) -> ActionResult {
	let motion_def = motion.def();

	trace!(
		motion = motion.name(),
		count = ctx.count,
		extend = ctx.extend,
		"Applying cursor motion"
	);

	let new_ranges: Vec<Range> = ctx
		.selection
		.ranges()
		.iter()
		.map(|range| {
			let seed = if ctx.extend {
				*range
			} else {
				Range::point(range.head)
			};
			let moved = (motion_def.handler)(ctx.text, seed, ctx.count, ctx.extend);
			if ctx.extend {
				moved
			} else {
				Range::point(moved.head)
			}
		})
		.collect();

	let sel = Selection::from_vec(new_ranges, ctx.selection.primary_index());
	ActionResult::Effects(ActionEffects::motion(sel))
}

/// Applies a typed motion as a selection-creating action.
///
/// Creates selections spanning from current positions to new positions
/// determined by the motion. When `ctx.extend` is true, extends all existing
/// selections; otherwise creates a single selection from the primary cursor.
///
/// Used for word motions (`w`, `b`, `e`) where the selection should span
/// from cursor to the motion target.
pub fn selection_motion(ctx: &ActionContext, motion: MotionKey) -> ActionResult {
	let motion_def = motion.def();

	trace!(
		motion = motion.name(),
		count = ctx.count,
		extend = ctx.extend,
		"Applying selection motion"
	);

	let sel = if ctx.extend {
		let primary_index = ctx.selection.primary_index();
		let new_ranges: Vec<Range> = ctx
			.selection
			.ranges()
			.iter()
			.enumerate()
			.map(|(i, range)| {
				let seed = if i == primary_index {
					Range::new(range.anchor, ctx.cursor)
				} else {
					*range
				};
				(motion_def.handler)(ctx.text, seed, ctx.count, true)
			})
			.collect();
		Selection::from_vec(new_ranges, primary_index)
	} else {
		let current_range = Range::point(ctx.cursor);
		let new_range = (motion_def.handler)(ctx.text, current_range, ctx.count, false);
		Selection::single(new_range.anchor, new_range.head)
	};

	ActionResult::Effects(ActionEffects::motion(sel))
}

/// Applies a typed motion as a word-selecting action (Kakoune/Helix style).
///
/// Each press selects a single word from cursor to motion target. With extend
/// (shift held), accumulates selection across multiple words instead.
pub fn word_motion(ctx: &ActionContext, motion: MotionKey) -> ActionResult {
	let motion_def = motion.def();

	trace!(
		motion = motion.name(),
		count = ctx.count,
		extend = ctx.extend,
		"Applying word motion"
	);

	let sel = if ctx.extend {
		let new_ranges: Vec<Range> = ctx
			.selection
			.ranges()
			.iter()
			.map(|range| (motion_def.handler)(ctx.text, *range, ctx.count, true))
			.collect();
		Selection::from_vec(new_ranges, ctx.selection.primary_index())
	} else {
		let current_range = Range::point(ctx.cursor);
		let new_range = (motion_def.handler)(ctx.text, current_range, ctx.count, false);
		Selection::single(new_range.anchor, new_range.head)
	};

	ActionResult::Effects(ActionEffects::motion(sel))
}

action!(move_left, {
	description: "Move cursor left",
	bindings: r#"normal "h" "left"
insert "left""#,
}, |ctx| cursor_motion(ctx, motions::left));

action!(move_right, {
	description: "Move cursor right",
	bindings: r#"normal "l" "right"
insert "right""#,
}, |ctx| cursor_motion(ctx, motions::right));

action!(move_line_start, { description: "Move to start of line", bindings: r#"normal "0" "home""# },
	|ctx| cursor_motion(ctx, motions::line_start));

action!(move_line_end, { description: "Move to end of line", bindings: r#"normal "$" "end""# },
	|ctx| cursor_motion(ctx, motions::line_end));

action!(next_word_start, {
	description: "Move to next word start",
	bindings: r#"normal "w" "ctrl-right"
insert "ctrl-right""#,
}, |ctx| word_motion(ctx, motions::next_word_start));

action!(prev_word_start, {
	description: "Move to previous word start",
	bindings: r#"normal "b" "ctrl-left"
insert "ctrl-left""#,
}, |ctx| word_motion(ctx, motions::prev_word_start));

action!(next_word_end, { description: "Move to next word end", bindings: r#"normal "e""# },
	|ctx| word_motion(ctx, motions::next_word_end));

action!(next_long_word_start, { description: "Move to next WORD start", bindings: r#"normal "W""# },
	|ctx| word_motion(ctx, motions::next_long_word_start));

action!(prev_long_word_start, { description: "Move to previous WORD start", bindings: r#"normal "B""# },
	|ctx| word_motion(ctx, motions::prev_long_word_start));

action!(next_long_word_end, { description: "Move to next WORD end", bindings: r#"normal "E""# },
	|ctx| word_motion(ctx, motions::next_long_word_end));

action!(select_word_forward, { description: "Select to next word start", bindings: r#"normal "alt-w""# },
	|ctx| selection_motion(ctx, motions::next_word_start));

action!(select_word_backward, { description: "Select to previous word start", bindings: r#"normal "alt-b""# },
	|ctx| selection_motion(ctx, motions::prev_word_start));

action!(select_word_end, { description: "Select to next word end", bindings: r#"normal "alt-e""# },
	|ctx| selection_motion(ctx, motions::next_word_end));

action!(next_paragraph, {
	description: "Move to next paragraph",
	bindings: r#"normal "}" "ctrl-down"
insert "ctrl-down""#,
}, |ctx| cursor_motion(ctx, motions::next_paragraph));

action!(prev_paragraph, {
	description: "Move to previous paragraph",
	bindings: r#"normal "{" "ctrl-up"
insert "ctrl-up""#,
}, |ctx| cursor_motion(ctx, motions::prev_paragraph));

action!(document_start, {
	description: "Goto file start",
	short_desc: "File start",
	bindings: r#"normal "g g""#,
}, |ctx| cursor_motion(ctx, motions::document_start));

action!(document_end, {
	description: "Goto file end",
	short_desc: "File end",
	bindings: r#"normal "g e" "G""#,
}, |ctx| cursor_motion(ctx, motions::document_end));

action!(goto_line_start, {
	description: "Goto line start",
	short_desc: "Line start",
	bindings: r#"normal "g h""#,
}, |ctx| cursor_motion(ctx, motions::line_start));

action!(goto_line_end, {
	description: "Goto line end",
	short_desc: "Line end",
	bindings: r#"normal "g l""#,
}, |ctx| cursor_motion(ctx, motions::line_end));

action!(goto_first_nonwhitespace, {
	description: "Goto first non-blank",
	short_desc: "First non-blank",
	bindings: r#"normal "g s""#,
}, |ctx| cursor_motion(ctx, motions::first_nonwhitespace));

action!(move_top_screen, { description: "Move to top of screen", bindings: r#"normal "H""# }, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Top, ctx.count))
});

action!(move_middle_screen, { description: "Move to middle of screen", bindings: r#"normal "M""# }, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Middle, ctx.count))
});

action!(move_bottom_screen, { description: "Move to bottom of screen" }, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Bottom, ctx.count))
});
