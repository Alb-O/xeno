//! Motion-based cursor movement actions.

use ropey::RopeSlice;
use tracing::trace;
use xeno_primitives::Selection;
use xeno_primitives::range::{CharIdx, Range};
use xeno_registry_motions::movement::is_word_char;
use xeno_registry_motions::{MotionKey, keys as motions};

use crate::{ActionContext, ActionEffects, ActionResult, ScreenPosition, action};

fn find_word_start(text: RopeSlice, pos: CharIdx) -> CharIdx {
	let mut start = pos;
	while start > 0 && text.get_char(start - 1).is_some_and(is_word_char) {
		start -= 1;
	}
	start
}

fn find_word_end(text: RopeSlice, pos: CharIdx) -> CharIdx {
	let len = text.len_chars();
	let mut end = pos;
	while end + 1 < len && text.get_char(end + 1).is_some_and(is_word_char) {
		end += 1;
	}
	end
}

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

/// Applies a word motion with boundary-aware selection semantics.
///
/// Selection behavior depends on cursor position and motion direction:
/// - Forward from word: selects to target, excluding next word's first char
/// - Backward, or non-word landing on word: selects just the target word
/// - Landing on non-word: moves cursor without selection
///
/// With extend (shift held), extends existing selection to the new position.
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
		let new_range = (motion_def.handler)(ctx.text, Range::point(ctx.cursor), ctx.count, false);
		let cursor_on_word = ctx.text.get_char(ctx.cursor).is_some_and(is_word_char);
		let target_on_word = ctx.text.get_char(new_range.head).is_some_and(is_word_char);
		let is_forward = new_range.head >= new_range.anchor;

		if cursor_on_word && is_forward {
			let at_boundary = !target_on_word
				|| new_range
					.head
					.checked_sub(1)
					.and_then(|p| ctx.text.get_char(p))
					.is_none_or(|c| !is_word_char(c));
			if at_boundary && new_range.head > new_range.anchor + 1 {
				Selection::single(new_range.anchor, new_range.head - 1)
			} else if at_boundary {
				Selection::point(new_range.head)
			} else {
				Selection::single(new_range.anchor, new_range.head)
			}
		} else if target_on_word {
			if is_forward {
				let word_start = find_word_start(ctx.text, new_range.head);
				Selection::single(word_start, new_range.head)
			} else {
				let word_end = find_word_end(ctx.text, new_range.head);
				Selection::single(word_end, new_range.head)
			}
		} else {
			Selection::point(new_range.head)
		}
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

action!(goto_next_hunk, {
	description: "Goto next diff hunk",
	short_desc: "Next hunk",
	bindings: r#"normal "] c""#,
}, |ctx| cursor_motion(ctx, motions::next_hunk));

action!(goto_prev_hunk, {
	description: "Goto previous diff hunk",
	short_desc: "Previous hunk",
	bindings: r#"normal "[ c""#,
}, |ctx| cursor_motion(ctx, motions::prev_hunk));
