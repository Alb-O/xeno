use evildoer_base::selection::Selection;

use crate::{action, ActionContext, ActionResult};

action!(collapse_selection, {
	description: "Collapse selection to cursor",
	bindings: r#"normal ";" "esc""#,
}, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| r.anchor = r.head);
	ActionResult::Motion(new_sel)
});

action!(flip_selection, {
	description: "Flip selection direction",
	bindings: r#"normal "alt-;""#,
}, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| std::mem::swap(&mut r.anchor, &mut r.head));
	ActionResult::Motion(new_sel)
});

action!(ensure_forward, {
	description: "Ensure selection is forward",
	bindings: r#"normal "alt-:""#,
}, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		if r.head < r.anchor {
			std::mem::swap(&mut r.anchor, &mut r.head);
		}
	});
	ActionResult::Motion(new_sel)
});

action!(select_line, {
	description: "Select current line",
	bindings: r#"normal "x""#,
}, handler: select_line_impl);

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

action!(select_all, {
	description: "Select all text",
	bindings: r#"normal "%""#,
}, |ctx| {
	let end = ctx.text.len_chars();
	ActionResult::Motion(Selection::single(0, end))
});

action!(expand_to_line, {
	description: "Expand selection to cover full lines",
	bindings: r#"normal "alt-x""#,
}, handler: expand_to_line_impl);

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

action!(remove_primary_selection, {
	description: "Remove the primary selection",
	bindings: r#"normal "alt-,""#,
}, |ctx| {
	if ctx.selection.len() <= 1 {
		return ActionResult::Ok;
	}
	let mut new_sel = ctx.selection.clone();
	new_sel.remove_primary();
	ActionResult::Motion(new_sel)
});

action!(remove_selections_except_primary, {
	description: "Remove all selections except the primary one",
	bindings: r#"normal ",""#,
}, |ctx| {
	ActionResult::Motion(Selection::single(
		ctx.selection.primary().anchor,
		ctx.selection.primary().head,
	))
});

action!(rotate_selections_forward, {
	description: "Rotate selections forward",
	bindings: r#"normal ")""#,
}, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.rotate_forward();
	ActionResult::Motion(new_sel)
});

action!(rotate_selections_backward, {
	description: "Rotate selections backward",
	bindings: r#"normal "(""#,
}, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.rotate_backward();
	ActionResult::Motion(new_sel)
});

action!(split_lines, { description: "Split selection into lines" }, |ctx| split_lines_impl(ctx));

fn split_lines_impl(ctx: &ActionContext) -> ActionResult {
	let text = &ctx.text;
	let mut new_ranges = Vec::new();

	for range in ctx.selection.ranges() {
		let from = range.min();
		let to = range.max();

		if from >= to {
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

action!(duplicate_selections_down, {
	description: "Duplicate selections on next lines",
	bindings: r#"normal "C" "+""#,
}, handler: duplicate_selections_down_impl);

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

action!(duplicate_selections_up, {
	description: "Duplicate selections on previous lines",
	bindings: r#"normal "alt-C""#,
}, handler: duplicate_selections_up_impl);

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

action!(merge_selections, {
	description: "Merge overlapping selections",
	bindings: r#"normal "alt-+""#,
}, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.merge_overlaps_and_adjacent();
	ActionResult::Motion(new_sel)
});

#[cfg(test)]
mod tests {
	use evildoer_base::{Rope, Selection};

	use super::*;
	use crate::ActionArgs;

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
			assert_eq!(
				primary.anchor, 7,
				"Anchor should move to start of next line"
			);
			assert_eq!(primary.head, 14, "Head should move to end of next line");
		} else {
			panic!("Expected Motion result");
		}
	}

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
