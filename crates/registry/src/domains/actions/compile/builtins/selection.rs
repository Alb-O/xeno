use xeno_primitives::Selection;

use crate::actions::{ActionEffects, ActionResult, action_handler};

action_handler!(collapse_selection, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| r.anchor = r.head);
	ActionResult::Effects(ActionEffects::selection(new_sel))
});

action_handler!(flip_selection, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| std::mem::swap(&mut r.anchor, &mut r.head));
	ActionResult::Effects(ActionEffects::selection(new_sel))
});

action_handler!(ensure_forward, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		if r.head < r.anchor {
			std::mem::swap(&mut r.anchor, &mut r.head);
		}
	});
	ActionResult::Effects(ActionEffects::selection(new_sel))
});

action_handler!(select_line, handler: select_line_impl);
action_handler!(extend_line, handler: extend_line_impl);

fn select_line_impl(ctx: &crate::actions::ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	let count = ctx.count.max(1);
	new_sel.transform_mut(|r| {
		let mut line = ctx.text.char_to_line(r.head);
		let line_start = ctx.text.line_to_char(line);
		let fully_selected = r.anchor == line_start && r.head == line_end_pos(ctx.text, line);
		if fully_selected && line + 1 < ctx.text.len_lines() {
			line += 1;
		}
		let target = (line + count - 1).min(ctx.text.len_lines().saturating_sub(1));
		r.anchor = ctx.text.line_to_char(line);
		r.head = line_end_pos(ctx.text, target);
	});
	ActionResult::Effects(ActionEffects::selection(new_sel))
}

fn extend_line_impl(ctx: &crate::actions::ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	let count = ctx.count.max(1);
	new_sel.transform_mut(|r| {
		let anchor_line = ctx.text.char_to_line(r.anchor);
		let head_line = ctx.text.char_to_line(r.head);
		let full_anchor = ctx.text.line_to_char(anchor_line);
		let full_head = line_end_pos(ctx.text, head_line);

		if r.anchor == full_anchor && r.head == full_head {
			let target = (head_line + count).min(ctx.text.len_lines().saturating_sub(1));
			r.head = line_end_pos(ctx.text, target);
		} else {
			r.anchor = full_anchor;
			r.head = full_head;
		}
	});
	ActionResult::Effects(ActionEffects::selection(new_sel))
}

fn line_end_pos(text: ropey::RopeSlice, line: usize) -> usize {
	if line + 1 < text.len_lines() {
		text.line_to_char(line + 1).saturating_sub(1)
	} else {
		text.len_chars().saturating_sub(1)
	}
}

action_handler!(select_all, |ctx| {
	let len = ctx.text.len_chars();
	if len == 0 {
		return ActionResult::Effects(ActionEffects::selection(Selection::point(0)));
	}
	let end = len - 1;
	ActionResult::Effects(ActionEffects::selection(Selection::single(0, end)))
});

action_handler!(expand_to_line, handler: expand_to_line_impl);

fn expand_to_line_impl(ctx: &crate::actions::ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		let start_line = ctx.text.char_to_line(r.min());
		let end_line = ctx.text.char_to_line(r.max());
		r.anchor = ctx.text.line_to_char(start_line);
		r.head = line_end_pos(ctx.text, end_line);
	});
	ActionResult::Effects(ActionEffects::selection(new_sel))
}

action_handler!(remove_primary_selection, |ctx| {
	if ctx.selection.len() <= 1 {
		return ActionResult::Effects(ActionEffects::ok());
	}
	let mut new_sel = ctx.selection.clone();
	new_sel.remove_primary();
	ActionResult::Effects(ActionEffects::selection(new_sel))
});

action_handler!(remove_selections_except_primary, |ctx| {
	ActionResult::Effects(ActionEffects::selection(Selection::single(
		ctx.selection.primary().anchor,
		ctx.selection.primary().head,
	)))
});

action_handler!(rotate_selections_forward, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.rotate_forward();
	ActionResult::Effects(ActionEffects::selection(new_sel))
});

action_handler!(rotate_selections_backward, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.rotate_backward();
	ActionResult::Effects(ActionEffects::selection(new_sel))
});

action_handler!(split_lines, |ctx| split_lines_impl(ctx));

fn split_lines_impl(ctx: &crate::actions::ActionContext) -> ActionResult {
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
				new_ranges.push(xeno_primitives::Range::new(line_start, line_end));
			}
		}
	}

	if new_ranges.is_empty() {
		ActionResult::Effects(ActionEffects::ok())
	} else {
		ActionResult::Effects(ActionEffects::selection(Selection::from_vec(new_ranges, 0)))
	}
}

action_handler!(duplicate_selections_down, handler: duplicate_selections_down_impl);

fn duplicate_selections_down_impl(ctx: &crate::actions::ActionContext) -> ActionResult {
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
		let new_range = xeno_primitives::Range::new(new_anchor, new_head);

		if !new_ranges.contains(&new_range) {
			new_ranges.push(new_range);
			if idx == primary_index {
				primary_index = new_ranges.len() - 1;
			}
		}
	}

	ActionResult::Effects(ActionEffects::selection(Selection::from_vec(new_ranges, primary_index)))
}

action_handler!(duplicate_selections_up, handler: duplicate_selections_up_impl);

fn duplicate_selections_up_impl(ctx: &crate::actions::ActionContext) -> ActionResult {
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
		let new_range = xeno_primitives::Range::new(new_anchor, new_head);

		if !new_ranges.contains(&new_range) {
			new_ranges.push(new_range);
			if idx == primary_index {
				primary_index = new_ranges.len() - 1;
			}
		}
	}

	ActionResult::Effects(ActionEffects::selection(Selection::from_vec(new_ranges, primary_index)))
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

action_handler!(merge_selections, |ctx| {
	let mut new_sel = ctx.selection.clone();
	new_sel.merge_overlaps_and_adjacent();
	ActionResult::Effects(ActionEffects::selection(new_sel))
});
