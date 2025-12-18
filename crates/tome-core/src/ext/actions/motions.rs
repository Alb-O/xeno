//! Motion actions that wrap MotionDefs into ActionDefs.

use linkme::distributed_slice;

use crate::ext::actions::{ACTIONS, ActionContext, ActionDef, ActionResult};
use crate::ext::find_motion;
use crate::range::Range;
use crate::selection::Selection;

/// Cursor movement - moves cursor (and all cursors) without creating new selections unless extending.
fn cursor_move_action(ctx: &ActionContext, motion_name: &str) -> ActionResult {
	let motion = match find_motion(motion_name) {
		Some(m) => m,
		None => return ActionResult::Error(format!("Unknown motion: {}", motion_name)),
	};

	let primary_index = ctx.selection.primary_index();

	// Move every selection head; when not extending, collapse to points at the new head.
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
			let moved = (motion.handler)(ctx.text, seed, ctx.count, ctx.extend);
			if ctx.extend {
				moved
			} else {
				Range::point(moved.head)
			}
		})
		.collect();

	ActionResult::Motion(Selection::from_vec(new_ranges, primary_index))
}

/// Selection-creating motion - creates new selection from old cursor to new position.
fn selection_motion_action(ctx: &ActionContext, motion_name: &str) -> ActionResult {
	let motion = match find_motion(motion_name) {
		Some(m) => m,
		None => return ActionResult::Error(format!("Unknown motion: {}", motion_name)),
	};

	// For selection-creating motions, we create a selection from cursor to new position
	if ctx.extend {
		// Extend each selection from its anchor using the detached cursor for the primary head
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
				(motion.handler)(ctx.text, seed, ctx.count, true)
			})
			.collect();
		ActionResult::Motion(crate::selection::Selection::from_vec(
			new_ranges,
			primary_index,
		))
	} else {
		// Otherwise start fresh from cursor
		let current_range = Range::point(ctx.cursor);
		let new_range = (motion.handler)(ctx.text, current_range, ctx.count, false);
		ActionResult::Motion(crate::selection::Selection::single(
			new_range.anchor,
			new_range.head,
		))
	}
}

macro_rules! cursor_action {
	($static_name:ident, $action_name:expr, $motion_name:expr, $description:expr) => {
		paste::paste! {
			fn [<handler_ $static_name>](ctx: &ActionContext) -> ActionResult {
				cursor_move_action(ctx, $motion_name)
			}

			#[distributed_slice(ACTIONS)]
			static [<ACTION_ $static_name:upper>]: ActionDef = ActionDef {
				name: $action_name,
				description: $description,
				handler: [<handler_ $static_name>],
			};
		}
	};
}

macro_rules! selection_action {
	($static_name:ident, $action_name:expr, $motion_name:expr, $description:expr) => {
		paste::paste! {
			fn [<handler_ $static_name>](ctx: &ActionContext) -> ActionResult {
				selection_motion_action(ctx, $motion_name)
			}

			#[distributed_slice(ACTIONS)]
			static [<ACTION_ $static_name:upper>]: ActionDef = ActionDef {
				name: $action_name,
				description: $description,
				handler: [<handler_ $static_name>],
			};
		}
	};
}

// Cursor movements - only move cursor, don't create selections
cursor_action!(action_move_left, "move_left", "move_left", "Move left");
cursor_action!(action_move_right, "move_right", "move_right", "Move right");
cursor_action!(action_move_up, "move_up", "move_up", "Move up");
cursor_action!(action_move_down, "move_down", "move_down", "Move down");
cursor_action!(
	action_move_line_start,
	"move_line_start",
	"line_start",
	"Move to line start"
);
cursor_action!(
	action_move_line_end,
	"move_line_end",
	"line_end",
	"Move to line end"
);
cursor_action!(
	action_move_first_nonblank,
	"move_first_nonblank",
	"first_nonwhitespace",
	"Move to first non-blank"
);
cursor_action!(
	action_document_start,
	"document_start",
	"document_start",
	"Move to document start"
);
cursor_action!(
	action_document_end,
	"document_end",
	"document_end",
	"Move to document end"
);

// Selection-creating motions - create selections
selection_action!(
	action_next_word_start,
	"next_word_start",
	"next_word_start",
	"Move to next word start"
);
selection_action!(
	action_next_word_end,
	"next_word_end",
	"next_word_end",
	"Move to next word end"
);
selection_action!(
	action_prev_word_start,
	"prev_word_start",
	"prev_word_start",
	"Move to previous word start"
);
selection_action!(
	action_prev_word_end,
	"prev_word_end",
	"prev_word_end",
	"Move to previous word end"
);
selection_action!(
	action_next_long_word_start,
	"next_long_word_start",
	"next_long_word_start",
	"Move to next WORD start"
);
selection_action!(
	action_next_long_word_end,
	"next_long_word_end",
	"next_long_word_end",
	"Move to next WORD end"
);
selection_action!(
	action_prev_long_word_start,
	"prev_long_word_start",
	"prev_long_word_start",
	"Move to previous WORD start"
);
