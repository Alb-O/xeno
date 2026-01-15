//! Motion application helpers.
//!
//! Functions for applying named motions to selections and cursors.
//!
//! # Effect-Based Variants
//!
//! The `effects_*` functions return [`ActionEffects`] instead of [`ActionResult`],
//! demonstrating the data-oriented composition pattern.

use tracing::trace;
use xeno_primitives::range::Range;
use xeno_primitives::{Mode, Selection};
use xeno_registry_motions::MotionKey;

use crate::{ActionContext, ActionEffects, ActionResult, Effect};

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

/// Applies a typed motion to all cursors before insert actions.
pub fn insert_with_motion(ctx: &ActionContext, motion: MotionKey) -> ActionResult {
	let motion_def = motion.def();
	let mut new_selection = ctx.selection.clone();
	new_selection.transform_mut(|range| {
		*range = (motion_def.handler)(ctx.text, *range, 1, false);
	});

	// Compose: SetSelection + SetMode instead of fused InsertWithMotion
	ActionResult::Effects(ActionEffects::motion(new_selection).with(Effect::SetMode(Mode::Insert)))
}
