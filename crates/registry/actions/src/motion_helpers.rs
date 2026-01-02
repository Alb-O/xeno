//! Motion application helpers.
//!
//! Functions for applying named motions to selections and cursors.

use evildoer_base::range::Range;
use evildoer_base::Selection;
use evildoer_registry_motions::find as find_motion;
use tracing::debug;

use crate::{ActionContext, ActionMode, ActionResult};

/// Applies a named motion as a cursor movement.
///
/// Looks up `motion_name` in the motion registry and applies it to each
/// cursor in the selection. When `ctx.extend` is false, collapses selections
/// to points at the new head positions.
///
/// Returns [`ActionResult::Error`] if the motion name is not found.
pub fn cursor_motion(ctx: &ActionContext, motion_name: &str) -> ActionResult {
	let Some(motion) = find_motion(motion_name) else {
		return ActionResult::Error(format!("Unknown motion: {}", motion_name));
	};

	debug!(
		motion = motion_name,
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
			let moved = (motion.handler)(ctx.text, seed, ctx.count, ctx.extend);
			if ctx.extend {
				moved
			} else {
				Range::point(moved.head)
			}
		})
		.collect();

	ActionResult::Motion(Selection::from_vec(
		new_ranges,
		ctx.selection.primary_index(),
	))
}

/// Applies a named motion as a selection-creating action.
///
/// Creates selections spanning from current positions to new positions
/// determined by the motion. When `ctx.extend` is true, extends all existing
/// selections; otherwise creates a single selection from the primary cursor.
///
/// Used for word motions (`w`, `b`, `e`) where the selection should span
/// from cursor to the motion target.
///
/// Returns [`ActionResult::Error`] if the motion name is not found.
pub fn selection_motion(ctx: &ActionContext, motion_name: &str) -> ActionResult {
	let Some(motion) = find_motion(motion_name) else {
		return ActionResult::Error(format!("Unknown motion: {}", motion_name));
	};

	debug!(
		motion = motion_name,
		count = ctx.count,
		extend = ctx.extend,
		"Applying selection motion"
	);

	if ctx.extend {
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
		ActionResult::Motion(Selection::from_vec(new_ranges, primary_index))
	} else {
		let current_range = Range::point(ctx.cursor);
		let new_range = (motion.handler)(ctx.text, current_range, ctx.count, false);
		ActionResult::Motion(Selection::single(new_range.anchor, new_range.head))
	}
}

/// Applies a named motion to all cursors, then enters insert mode.
///
/// Falls back to plain insert mode if the motion is not found.
pub fn insert_with_motion(ctx: &ActionContext, motion_name: &str) -> ActionResult {
	let Some(motion) = find_motion(motion_name) else {
		return ActionResult::ModeChange(ActionMode::Insert);
	};

	let mut new_selection = ctx.selection.clone();
	new_selection.transform_mut(|range| {
		*range = (motion.handler)(ctx.text, *range, 1, false);
	});

	ActionResult::InsertWithMotion(new_selection)
}
