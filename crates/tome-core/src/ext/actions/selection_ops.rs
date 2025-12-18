//! Selection manipulation actions (collapse, flip, select all, etc.).

use linkme::distributed_slice;
use smallvec::SmallVec;

use crate::ext::actions::{ACTIONS, ActionContext, ActionDef, ActionResult};
use crate::graphemes::prev_grapheme_boundary;
use crate::range::Range;
use crate::selection::Selection;

fn collapse_selection(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		r.anchor = r.head;
	});
	ActionResult::Motion(new_sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_COLLAPSE_SELECTION: ActionDef = ActionDef {
	name: "collapse_selection",
	description: "Collapse selection to cursor",
	handler: collapse_selection,
};

fn flip_selection(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		std::mem::swap(&mut r.anchor, &mut r.head);
	});
	ActionResult::Motion(new_sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_FLIP_SELECTION: ActionDef = ActionDef {
	name: "flip_selection",
	description: "Flip selection direction",
	handler: flip_selection,
};

fn ensure_forward(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		if r.head < r.anchor {
			std::mem::swap(&mut r.anchor, &mut r.head);
		}
	});
	ActionResult::Motion(new_sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_ENSURE_FORWARD: ActionDef = ActionDef {
	name: "ensure_forward",
	description: "Ensure selection is forward",
	handler: ensure_forward,
};

fn select_line(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		let line = ctx.text.char_to_line(r.head);
		let start = ctx.text.line_to_char(line);
		let end = if line + 1 < ctx.text.len_lines() {
			ctx.text.line_to_char(line + 1)
		} else {
			ctx.text.len_chars()
		};
		r.anchor = start;
		r.head = end;
	});
	ActionResult::Motion(new_sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_SELECT_LINE: ActionDef = ActionDef {
	name: "select_line",
	description: "Select whole line",
	handler: select_line,
};

fn select_all(ctx: &ActionContext) -> ActionResult {
	ActionResult::Motion(Selection::single(0, ctx.text.len_chars()))
}

#[distributed_slice(ACTIONS)]
static ACTION_SELECT_ALL: ActionDef = ActionDef {
	name: "select_all",
	description: "Select entire buffer",
	handler: select_all,
};

fn keep_primary_selection(ctx: &ActionContext) -> ActionResult {
	let primary = ctx.selection.primary();
	ActionResult::Motion(Selection::single(primary.anchor, primary.head))
}

#[distributed_slice(ACTIONS)]
static ACTION_KEEP_PRIMARY: ActionDef = ActionDef {
	name: "keep_primary_selection",
	description: "Keep only primary selection",
	handler: keep_primary_selection,
};

fn rotate_selections_forward(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.rotate_forward();
	ActionResult::Motion(new_sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_ROTATE_FORWARD: ActionDef = ActionDef {
	name: "rotate_selections_forward",
	description: "Rotate selections forward",
	handler: rotate_selections_forward,
};

fn rotate_selections_backward(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.rotate_backward();
	ActionResult::Motion(new_sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_ROTATE_BACKWARD: ActionDef = ActionDef {
	name: "rotate_selections_backward",
	description: "Rotate selections backward",
	handler: rotate_selections_backward,
};

fn escape(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		r.anchor = r.head;
	});
	ActionResult::Motion(new_sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_ESCAPE: ActionDef = ActionDef {
	name: "escape",
	description: "Escape (collapse selection)",
	handler: escape,
};

fn remove_primary_selection(ctx: &ActionContext) -> ActionResult {
	if ctx.selection.ranges().len() <= 1 {
		return ActionResult::Ok;
	}
	let mut new_sel = ctx.selection.clone();
	new_sel.remove_primary();
	ActionResult::Motion(new_sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_REMOVE_PRIMARY: ActionDef = ActionDef {
	name: "remove_primary_selection",
	description: "Remove primary selection",
	handler: remove_primary_selection,
};

fn merge_selections(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.merge_overlaps_and_adjacent();
	ActionResult::Motion(new_sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_MERGE_SELECTIONS: ActionDef = ActionDef {
	name: "merge_selections",
	description: "Merge overlapping or adjacent selections explicitly",
	handler: merge_selections,
};

fn split_selection_lines(ctx: &ActionContext) -> ActionResult {
	let mut ranges = SmallVec::<[Range; 4]>::new();
	let primary = ctx.selection.primary();
	let primary_line = ctx.text.char_to_line(primary.head);

	for r in ctx.selection.ranges().iter() {
		let start = r.from().min(r.to());
		let end = r.from().max(r.to());
		let start_line = ctx.text.char_to_line(start);
		let end_line = ctx.text.char_to_line(end);

		for line in start_line..=end_line {
			let line_start = ctx.text.line_to_char(line);
			let next_line_start = if line + 1 < ctx.text.len_lines() {
				ctx.text.line_to_char(line + 1)
			} else {
				ctx.text.len_chars()
			};

			// Exclude newline from selection so head stays on the current line.
			// If we include the newline, the cursor head would be at the start of the *next* line,
			// which causes insertions to happen on the wrong line.
			let line_end = if line + 1 < ctx.text.len_lines() {
				prev_grapheme_boundary(ctx.text, next_line_start)
			} else {
				next_line_start
			};

			// Create a backward selection (head at start, anchor at end).
			// This places the cursor at the beginning of the line, which is typically desired
			// when splitting lines (e.g., for block editing or indentation).
			ranges.push(Range::new(line_end, line_start));
		}
	}

	if ranges.is_empty() {
		return ActionResult::Ok;
	}

	let mut sel = Selection::from_vec(ranges.into_vec(), 0);
	// Set primary to the range on the original primary line if present
	let primary_idx = sel
		.ranges()
		.iter()
		.position(|r| {
			let line = ctx.text.char_to_line(r.head);
			line == primary_line
		})
		.unwrap_or(0);
	sel.set_primary(primary_idx);
	ActionResult::Motion(sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_SPLIT_SELECTION_LINES: ActionDef = ActionDef {
	name: "split_selection_lines",
	description: "Split selections into per-line selections (multi-cursor lines)",
	handler: split_selection_lines,
};

fn clone_selections_to_matches(ctx: &ActionContext) -> ActionResult {
	let primary = ctx.selection.primary();
	let from = primary.from();
	let to = primary.to();
	if from == to {
		return ActionResult::Ok;
	}

	let pattern: String = ctx.text.slice(from..to).to_string();
	if pattern.is_empty() {
		return ActionResult::Ok;
	}

	let haystack: String = ctx.text.to_string();
	let mut ranges = SmallVec::<[Range; 4]>::new();

	let mut search_start = 0;
	let pattern_bytes = pattern.as_bytes();
	let mut primary_index = 0usize;

	while let Some(idx) = haystack[search_start..].find(&pattern) {
		let abs_byte = search_start + idx;
		let start_char = haystack[..abs_byte].chars().count();
		let pat_len_chars = pattern.chars().count();
		let end_char = start_char + pat_len_chars;
		let r = Range::new(start_char, end_char);
		if r.from() == from && r.to() == to {
			primary_index = ranges.len();
		}
		ranges.push(r);
		// Advance past this match (non-overlapping)
		search_start = abs_byte + pattern_bytes.len();
	}

	if ranges.is_empty() {
		return ActionResult::Ok;
	}

	let sel = Selection::from_vec(ranges.into_vec(), primary_index);
	ActionResult::Motion(sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_CLONE_SELECTIONS_TO_MATCHES: ActionDef = ActionDef {
	name: "clone_selections_to_matches",
	description: "Clone primary selection to all exact matches in buffer",
	handler: clone_selections_to_matches,
};

fn trim_to_line(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		let line = ctx.text.char_to_line(r.head);
		let start = ctx.text.line_to_char(line);
		let end = if line + 1 < ctx.text.len_lines() {
			ctx.text.line_to_char(line + 1).saturating_sub(1)
		} else {
			ctx.text.len_chars()
		};
		r.anchor = r.anchor.max(start).min(end);
		r.head = r.head.max(start).min(end);
	});
	ActionResult::Motion(new_sel)
}

#[distributed_slice(ACTIONS)]
static ACTION_TRIM_TO_LINE: ActionDef = ActionDef {
	name: "trim_to_line",
	description: "Trim selection to line boundaries",
	handler: trim_to_line,
};
