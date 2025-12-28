//! Selection manipulation actions (collapse, flip, select all, etc.).

use evildoer_base::key::{Key, SpecialKey};
use evildoer_base::selection::Selection;
use evildoer_manifest::actions::{ActionContext, ActionResult};
use evildoer_manifest::bound_action;

use crate::action;

bound_action!(
	collapse_selection,
	description: "Collapse selection to cursor",
	bindings: [Normal => [Key::char(';'), Key::special(SpecialKey::Escape)]],
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| r.anchor = r.head);
		ActionResult::Motion(new_sel)
	}
);

bound_action!(
	flip_selection,
	description: "Flip selection direction",
	bindings: [Normal => [Key::alt(';')]],
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| std::mem::swap(&mut r.anchor, &mut r.head));
		ActionResult::Motion(new_sel)
	}
);

bound_action!(
	ensure_forward,
	description: "Ensure selection is forward",
	bindings: [Normal => [Key::alt(':')]],
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| {
			if r.head < r.anchor {
				std::mem::swap(&mut r.anchor, &mut r.head);
			}
		});
		ActionResult::Motion(new_sel)
	}
);

bound_action!(
	select_line,
	description: "Select current line",
	bindings: [Normal => [Key::char('x')]],
	handler: select_line_impl
);

fn select_line_impl(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	let count = ctx.count.max(1);
	new_sel.transform_mut(|r| {
		let line = ctx.text.char_to_line(r.head);
		let start = ctx.text.line_to_char(line);
		let end = if line + count < ctx.text.len_lines() {
			ctx.text.line_to_char(line + count)
		} else {
			ctx.text.len_chars()
		};

		if ctx.extend {
			r.head = end;
		} else {
			r.anchor = start;
			r.head = end;
		}
	});
	ActionResult::Motion(new_sel)
}

bound_action!(
	select_all,
	description: "Select all text",
	bindings: [Normal => [Key::char('%')]],
	|ctx| {
		let end = ctx.text.len_chars();
		ActionResult::Motion(Selection::single(0, end))
	}
);

bound_action!(
	expand_to_line,
	description: "Expand selection to cover full lines",
	bindings: [Normal => [Key::alt('x')]],
	handler: expand_to_line_impl
);

fn expand_to_line_impl(ctx: &ActionContext) -> ActionResult {
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

bound_action!(
	remove_primary_selection,
	description: "Remove the primary selection",
	bindings: [Normal => [Key::alt(',')]],
	|ctx| {
		if ctx.selection.len() <= 1 {
			return ActionResult::Ok;
		}
		let mut new_sel = ctx.selection.clone();
		new_sel.remove_primary();
		ActionResult::Motion(new_sel)
	}
);

bound_action!(
	remove_selections_except_primary,
	description: "Remove all selections except the primary one",
	bindings: [Normal => [Key::char(',')]],
	|ctx| {
		ActionResult::Motion(Selection::single(
			ctx.selection.primary().anchor,
			ctx.selection.primary().head,
		))
	}
);

bound_action!(
	rotate_selections_forward,
	description: "Rotate selections forward",
	bindings: [Normal => [Key::char(')')]],
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.rotate_forward();
		ActionResult::Motion(new_sel)
	}
);

bound_action!(
	rotate_selections_backward,
	description: "Rotate selections backward",
	bindings: [Normal => [Key::char('(')]],
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.rotate_backward();
		ActionResult::Motion(new_sel)
	}
);

action!(
	split_lines,
	{ description: "Split selection into lines" },
	handler: split_lines_impl
);

fn split_lines_impl(ctx: &ActionContext) -> ActionResult {
	let text = &ctx.text;
	let mut new_ranges = Vec::new();

	for range in ctx.selection.ranges() {
		let from = range.min();
		let to = range.max();

		if from >= to {
			// Keep collapsed selections as-is
			new_ranges.push(*range);
			continue;
		}

		let start_line = text.char_to_line(from);
		let end_line = text.char_to_line(to.saturating_sub(1));

		for line in start_line..=end_line {
			let line_start = text.line_to_char(line).max(from);
			let line_end = if line + 1 < text.len_lines() {
				text.line_to_char(line + 1).min(to)
			} else {
				text.len_chars().min(to)
			};

			if line_start < line_end {
				new_ranges.push(evildoer_base::range::Range::new(line_start, line_end));
			}
		}
	}

	if new_ranges.is_empty() {
		ActionResult::Ok
	} else {
		ActionResult::Motion(Selection::from_vec(new_ranges, 0))
	}
}

bound_action!(
	duplicate_selections_down,
	description: "Duplicate selections on next lines",
	bindings: [Normal => [Key::char('C'), Key::char('+')]],
	handler: duplicate_selections_down_impl
);

fn duplicate_selections_down_impl(ctx: &ActionContext) -> ActionResult {
	let text = &ctx.text;
	let mut new_ranges = ctx.selection.ranges().to_vec();
	let mut primary_index = ctx.selection.primary_index();

	for (idx, range) in ctx.selection.ranges().iter().enumerate() {
		let anchor_line = text.char_to_line(range.anchor);
		let head_line = text.char_to_line(range.head);
		let target_anchor_line = anchor_line + 1;
		let target_head_line = head_line + 1;

		if target_anchor_line >= text.len_lines() || target_head_line >= text.len_lines() {
			continue;
		}

		let anchor_col = range.anchor - text.line_to_char(anchor_line);
		let head_col = range.head - text.line_to_char(head_line);

		let new_anchor = line_col_to_char(text, target_anchor_line, anchor_col);
		let new_head = line_col_to_char(text, target_head_line, head_col);
		let new_range = evildoer_base::range::Range::new(new_anchor, new_head);

		if !new_ranges.contains(&new_range) {
			new_ranges.push(new_range);
			if idx == primary_index {
				primary_index = new_ranges.len() - 1;
			}
		}
	}

	ActionResult::Motion(Selection::from_vec(new_ranges, primary_index))
}

bound_action!(
	duplicate_selections_up,
	description: "Duplicate selections on previous lines",
	bindings: [Normal => [Key::alt('C')]],
	handler: duplicate_selections_up_impl
);

fn duplicate_selections_up_impl(ctx: &ActionContext) -> ActionResult {
	let text = &ctx.text;
	let mut new_ranges = ctx.selection.ranges().to_vec();
	let mut primary_index = ctx.selection.primary_index();

	for (idx, range) in ctx.selection.ranges().iter().enumerate() {
		let anchor_line = text.char_to_line(range.anchor);
		let head_line = text.char_to_line(range.head);

		if anchor_line == 0 || head_line == 0 {
			continue;
		}

		let target_anchor_line = anchor_line - 1;
		let target_head_line = head_line - 1;

		let anchor_col = range.anchor - text.line_to_char(anchor_line);
		let head_col = range.head - text.line_to_char(head_line);

		let new_anchor = line_col_to_char(text, target_anchor_line, anchor_col);
		let new_head = line_col_to_char(text, target_head_line, head_col);
		let new_range = evildoer_base::range::Range::new(new_anchor, new_head);

		if !new_ranges.contains(&new_range) {
			new_ranges.push(new_range);
			if idx == primary_index {
				primary_index = new_ranges.len() - 1;
			}
		}
	}

	ActionResult::Motion(Selection::from_vec(new_ranges, primary_index))
}

/// Convert line and column to char index, clamping column to line length.
fn line_col_to_char(text: &ropey::RopeSlice, line: usize, col: usize) -> usize {
	let line_start = text.line_to_char(line);
	let line_end = if line + 1 < text.len_lines() {
		text.line_to_char(line + 1)
	} else {
		text.len_chars()
	};
	let line_len = line_end.saturating_sub(line_start);
	line_start + col.min(line_len)
}

bound_action!(
	merge_selections,
	description: "Merge overlapping selections",
	bindings: [Normal => [Key::alt('+')]],
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.merge_overlaps_and_adjacent();
		ActionResult::Motion(new_sel)
	}
);

#[cfg(test)]
mod tests {
	use evildoer_manifest::actions::ActionArgs;

	use super::*;
	use crate::{Rope, Selection};

	/// Tests line selection with extend mode starting from partial line selection.
	#[test]
	fn test_select_line_extend() {
		let text = Rope::from("line 1\nline 2\nline 3\n");
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

		let result = select_line_impl(&ctx);
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

	/// Tests repeated line selection replaces with next line in normal mode.
	#[test]
	fn test_select_line_repeated() {
		let text = Rope::from("line 1\nline 2\nline 3\n");
		let sel = Selection::single(0, 7);

		let ctx = ActionContext {
			text: text.slice(..),
			cursor: 7,
			selection: &sel,
			count: 1,
			extend: false,
			register: None,
			args: ActionArgs::default(),
		};

		let result = select_line_impl(&ctx);
		if let ActionResult::Motion(new_sel) = result {
			let primary = new_sel.primary();
			// Should replace with line 2
			assert_eq!(
				primary.anchor, 7,
				"Anchor should move to start of next line (replace behavior)"
			);
			assert_eq!(primary.head, 14, "Head should move to end of next line");
		} else {
			panic!("Expected Motion result");
		}
	}

	/// Tests line selection with count selects multiple lines.
	#[test]
	fn test_select_line_count() {
		let text = Rope::from("line 1\nline 2\nline 3\n");
		let sel = Selection::point(0);

		let ctx = ActionContext {
			text: text.slice(..),
			cursor: 0,
			selection: &sel,
			count: 2,
			extend: false,
			register: None,
			args: ActionArgs::default(),
		};

		let result = select_line_impl(&ctx);
		if let ActionResult::Motion(new_sel) = result {
			let primary = new_sel.primary();
			assert_eq!(primary.anchor, 0);
			assert_eq!(primary.head, 14, "should select 2 complete lines");
		} else {
			panic!("Expected Motion result");
		}
	}
}
