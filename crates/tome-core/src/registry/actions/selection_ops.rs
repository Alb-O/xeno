//! Selection manipulation actions (collapse, flip, select all, etc.).

use crate::action;
use crate::registry::actions::{ActionContext, ActionResult};
use crate::range::Range;
use crate::selection::Selection;

action!(collapse_selection, { description: "Collapse selection to cursor" }, handler: collapse_selection);

fn collapse_selection(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		r.anchor = r.head;
	});
	ActionResult::Motion(new_sel)
}

action!(flip_selection, { description: "Flip selection direction" }, handler: flip_selection);

fn flip_selection(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		std::mem::swap(&mut r.anchor, &mut r.head);
	});
	ActionResult::Motion(new_sel)
}

action!(ensure_forward, { description: "Ensure selection is forward" }, handler: ensure_forward);

fn ensure_forward(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		if r.head < r.anchor {
			std::mem::swap(&mut r.anchor, &mut r.head);
		}
	});
	ActionResult::Motion(new_sel)
}

action!(select_line, { description: "Select current line" }, handler: select_line);

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

		if ctx.extend {
			r.head = end;
		} else {
			// Check if we are already selecting exactly one or more full lines forward.
			let is_full_line = !r.is_empty()
				&& r.anchor <= r.head
				&& r.anchor == ctx.text.line_to_char(ctx.text.char_to_line(r.anchor))
				&& r.head
					== if ctx.text.char_to_line(r.head) < ctx.text.len_lines() {
						ctx.text.line_to_char(ctx.text.char_to_line(r.head))
					} else {
						ctx.text.len_chars()
					};

			if is_full_line {
				r.head = end;
			} else {
				r.anchor = start;
				r.head = end;
			}
		}
	});
	ActionResult::Motion(new_sel)
}

action!(select_all, { description: "Select all text" }, |ctx| {
	let end = ctx.text.len_chars();
	ActionResult::Motion(Selection::single(0, end))
});

action!(expand_to_line, { description: "Expand selection to cover full lines" }, handler: expand_to_line);

fn expand_to_line(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		let start_line = ctx.text.char_to_line(r.min());
		let end_line = ctx.text.char_to_line(r.max());
		r.anchor = ctx.text.line_to_char(start_line);
		r.head = if end_line + 1 < ctx.text.len_lines() {
			ctx.text.line_to_char(end_line + 1)
		} else {
			ctx.text.len_chars()
		};
	});
	ActionResult::Motion(new_sel)
}

action!(trim_selection, { description: "Trim whitespace from selection" }, handler: trim_selection);

fn trim_selection(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		if r.is_empty() {
			return;
		}
		let mut start = r.min();
		let mut end = r.max();
		while start < end && ctx.text.char(start).is_whitespace() {
			start += 1;
		}
		while end > start && ctx.text.char(end - 1).is_whitespace() {
			end -= 1;
		}
		if r.anchor <= r.head {
			r.anchor = start;
			r.head = end;
		} else {
			r.anchor = end;
			r.head = start;
		}
	});
	ActionResult::Motion(new_sel)
}

action!(split_selection_on_newline, { description: "Split multi-line selections into one per line" }, handler: split_selection_on_newline);

fn split_selection_on_newline(ctx: &ActionContext) -> ActionResult {
	let mut new_ranges = Vec::new();
	for r in ctx.selection.ranges() {
		let start_line = ctx.text.char_to_line(r.min());
		let end_line = ctx.text.char_to_line(r.max());
		if start_line == end_line {
			new_ranges.push(*r);
			continue;
		}
		for line in start_line..=end_line {
			let line_start = ctx.text.line_to_char(line);
			let line_end = if line + 1 < ctx.text.len_lines() {
				ctx.text.line_to_char(line + 1)
			} else {
				ctx.text.len_chars()
			};
			let sel_start = std::cmp::max(r.min(), line_start);
			let sel_end = std::cmp::min(r.max(), line_end);
			if sel_start < sel_end {
				new_ranges.push(Range::new(sel_start, sel_end));
			}
		}
	}
	if new_ranges.is_empty() {
		return ActionResult::Ok;
	}
	let primary = new_ranges[0];
	ActionResult::Motion(Selection::new(primary, new_ranges.into_iter().skip(1)))
}

action!(merge_overlapping, { description: "Merge overlapping selections" }, |ctx| {
	let mut sel = ctx.selection.clone();
	sel.merge_overlaps_and_adjacent();
	ActionResult::Motion(sel)
});

action!(remove_primary_selection, { description: "Remove the primary selection" }, handler: remove_primary_selection);

fn remove_primary_selection(ctx: &ActionContext) -> ActionResult {
	if ctx.selection.len() <= 1 {
		return ActionResult::Ok;
	}
	let mut new_sel = ctx.selection.clone();
	new_sel.remove_primary();
	ActionResult::Motion(new_sel)
}

action!(
	remove_selections_except_primary,
	{ description: "Remove all selections except the primary one" },
	|ctx| {
		ActionResult::Motion(Selection::single(
			ctx.selection.primary().anchor,
			ctx.selection.primary().head,
		))
	}
);

action!(add_cursor_above, { description: "Add a cursor on the line above" }, handler: add_cursor_above);

fn add_cursor_above(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	let primary = new_sel.primary();
	let line = ctx.text.char_to_line(primary.head);
	if line == 0 {
		return ActionResult::Ok;
	}
	let col = primary.head - ctx.text.line_to_char(line);
	let prev_line_start = ctx.text.line_to_char(line - 1);
	let prev_line_end = ctx.text.line_to_char(line);
	let new_head = std::cmp::min(prev_line_start + col, prev_line_end.saturating_sub(1));
	new_sel.push(Range::point(new_head));
	ActionResult::Motion(new_sel)
}

action!(add_cursor_below, { description: "Add a cursor on the line below" }, handler: add_cursor_below);

fn add_cursor_below(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	let primary = new_sel.primary();
	let line = ctx.text.char_to_line(primary.head);
	if line + 1 >= ctx.text.len_lines() {
		return ActionResult::Ok;
	}
	let col = primary.head - ctx.text.line_to_char(line);
	let next_line_start = ctx.text.line_to_char(line + 1);
	let next_line_end = if line + 2 < ctx.text.len_lines() {
		ctx.text.line_to_char(line + 2)
	} else {
		ctx.text.len_chars()
	};
	let new_head = std::cmp::min(next_line_start + col, next_line_end.saturating_sub(1));
	new_sel.push(Range::point(new_head));
	ActionResult::Motion(new_sel)
}

action!(
	rotate_selections_forward,
	{ description: "Rotate selections forward" },
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.rotate_forward();
		ActionResult::Motion(new_sel)
	}
);

action!(
	rotate_selections_backward,
	{ description: "Rotate selections backward" },
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.rotate_backward();
		ActionResult::Motion(new_sel)
	}
);

#[cfg(test)]
mod tests {
	use super::*;
	use crate::registry::actions::ActionArgs;
	use crate::{Rope, Selection};

	#[test]
	fn test_select_line_extend() {
		let text = Rope::from("line 1\nline 2\nline 3\n");
		// Select 'ine 1'
		let sel = Selection::single(1, 6);

		let ctx = ActionContext {
			text: text.slice(..),
			cursor: 6,
			selection: &sel,
			count: 1,
			extend: true,
			register: None,
			args: ActionArgs::default(),
		};

		let result = select_line(&ctx);
		if let ActionResult::Motion(new_sel) = result {
			let primary = new_sel.primary();
			// If it extends, anchor should stay at 1, head should be start of next line (7)
			assert_eq!(
				primary.anchor, 1,
				"Anchor should be preserved when extending"
			);
			assert_eq!(primary.head, 7, "Head should be at end of line");
		} else {
			panic!("Expected Motion result");
		}
	}

	#[test]
	fn test_select_line_repeated() {
		let text = Rope::from("line 1\nline 2\nline 3\n");
		// Select line 1
		let sel = Selection::single(0, 7);

		let ctx = ActionContext {
			text: text.slice(..),
			cursor: 7,
			selection: &sel,
			count: 1,
			extend: false, // Normal 'x'
			register: None,
			args: ActionArgs::default(),
		};

		let result = select_line(&ctx);
		if let ActionResult::Motion(new_sel) = result {
			let primary = new_sel.primary();
			// Should extend to include line 2
			assert_eq!(
				primary.anchor, 0,
				"Anchor should be preserved when already full line"
			);
			assert_eq!(primary.head, 14, "Head should move to end of next line");
		} else {
			panic!("Expected Motion result");
		}
	}
}
