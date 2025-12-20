//! Selection manipulation actions (collapse, flip, select all, etc.).

use crate::action;
use crate::ext::actions::{ActionContext, ActionResult};
use crate::range::Range;
use crate::selection::Selection;

action!(collapse_selection, "Collapse selection to cursor", handler: collapse_selection);

fn collapse_selection(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		r.anchor = r.head;
	});
	ActionResult::Motion(new_sel)
}

action!(flip_selection, "Flip selection direction", handler: flip_selection);

fn flip_selection(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		std::mem::swap(&mut r.anchor, &mut r.head);
	});
	ActionResult::Motion(new_sel)
}

action!(ensure_forward, "Ensure selection is forward", handler: ensure_forward);

fn ensure_forward(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		if r.head < r.anchor {
			std::mem::swap(&mut r.anchor, &mut r.head);
		}
	});
	ActionResult::Motion(new_sel)
}

action!(select_line, "Select current line", handler: select_line);

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

action!(select_all, "Select all text", |ctx| {
	let end = ctx.text.len_chars();
	ActionResult::Motion(Selection::single(0, end))
});

action!(expand_to_line, "Expand selection to cover full lines", handler: expand_to_line);

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

action!(trim_selection, "Trim whitespace from selection", handler: trim_selection);

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

action!(split_selection_on_newline, "Split multi-line selections into one per line", handler: split_selection_on_newline);

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

action!(merge_overlapping, "Merge overlapping selections", |ctx| {
	let mut sel = ctx.selection.clone();
	sel.merge_overlaps_and_adjacent();
	ActionResult::Motion(sel)
});

action!(remove_primary_selection, "Remove the primary selection", handler: remove_primary_selection);

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
	"Remove all selections except the primary one",
	|ctx| {
		ActionResult::Motion(Selection::single(
			ctx.selection.primary().anchor,
			ctx.selection.primary().head,
		))
	}
);

action!(add_cursor_above, "Add a cursor on the line above", handler: add_cursor_above);

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

action!(add_cursor_below, "Add a cursor on the line below", handler: add_cursor_below);

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
	"Rotate selections forward",
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.rotate_forward();
		ActionResult::Motion(new_sel)
	}
);

action!(
	rotate_selections_backward,
	"Rotate selections backward",
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.rotate_backward();
		ActionResult::Motion(new_sel)
	}
);
