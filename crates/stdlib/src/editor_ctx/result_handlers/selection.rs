//! Result handlers for selection-oriented operations.

use tome_manifest::actions::ActionResult;
use tome_manifest::editor_ctx::HandleOutcome;
use tome_manifest::result_handler;

use crate::NotifyWARNExt;

result_handler!(
	RESULT_SPLIT_LINES_HANDLERS,
	HANDLE_SPLIT_LINES,
	"split_lines",
	|_, ctx, _| {
		if let Some(ops) = ctx.selection_ops() {
			ops.split_lines();
			HandleOutcome::Handled
		} else {
			ctx.warn("Split lines not available");
			HandleOutcome::Handled
		}
	}
);

result_handler!(
	RESULT_DUPLICATE_SELECTIONS_DOWN_HANDLERS,
	HANDLE_DUPLICATE_DOWN,
	"duplicate_down",
	|r, ctx, _| {
		if let ActionResult::DuplicateSelectionsDown = r {
			let text = ctx.text();
			let mut new_ranges = ctx.selection().ranges().to_vec();
			let mut primary_index = ctx.selection().primary_index();

			for (idx, range) in ctx.selection().ranges().iter().enumerate() {
				let anchor_line = text.char_to_line(range.anchor);
				let head_line = text.char_to_line(range.head);
				let target_anchor_line = anchor_line + 1;
				let target_head_line = head_line + 1;
				if target_anchor_line >= text.len_lines() || target_head_line >= text.len_lines() {
					continue;
				}
				let anchor_col = range.anchor - text.line_to_char(anchor_line);
				let head_col = range.head - text.line_to_char(head_line);

				let anchor_start = text.line_to_char(target_anchor_line);
				let anchor_end = if target_anchor_line + 1 < text.len_lines() {
					text.line_to_char(target_anchor_line + 1)
				} else {
					text.len_chars()
				};
				let head_start = text.line_to_char(target_head_line);
				let head_end = if target_head_line + 1 < text.len_lines() {
					text.line_to_char(target_head_line + 1)
				} else {
					text.len_chars()
				};

				let anchor_line_len = anchor_end.saturating_sub(anchor_start);
				let head_line_len = head_end.saturating_sub(head_start);

				let new_anchor = anchor_start + anchor_col.min(anchor_line_len);
				let new_head = head_start + head_col.min(head_line_len);
				let new_range = tome_base::range::Range::new(new_anchor, new_head);

				if new_ranges.contains(&new_range) {
					continue;
				}

				new_ranges.push(new_range);
				if idx == primary_index {
					primary_index = new_ranges.len() - 1;
				}
			}

			let sel = tome_base::selection::Selection::from_vec(new_ranges, primary_index);
			ctx.set_selection(sel.clone());
			ctx.set_cursor(sel.primary().head);
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_DUPLICATE_SELECTIONS_UP_HANDLERS,
	HANDLE_DUPLICATE_UP,
	"duplicate_up",
	|r, ctx, _| {
		if let ActionResult::DuplicateSelectionsUp = r {
			let text = ctx.text();
			let mut new_ranges = ctx.selection().ranges().to_vec();
			let mut primary_index = ctx.selection().primary_index();

			for (idx, range) in ctx.selection().ranges().iter().enumerate() {
				let anchor_line = text.char_to_line(range.anchor);
				let head_line = text.char_to_line(range.head);
				if anchor_line == 0 || head_line == 0 {
					continue;
				}
				let target_anchor_line = anchor_line - 1;
				let target_head_line = head_line - 1;

				let anchor_col = range.anchor - text.line_to_char(anchor_line);
				let head_col = range.head - text.line_to_char(head_line);

				let anchor_start = text.line_to_char(target_anchor_line);
				let anchor_end = text.line_to_char(target_anchor_line + 1);
				let head_start = text.line_to_char(target_head_line);
				let head_end = text.line_to_char(target_head_line + 1);

				let anchor_line_len = anchor_end.saturating_sub(anchor_start);
				let head_line_len = head_end.saturating_sub(head_start);

				let new_anchor = anchor_start + anchor_col.min(anchor_line_len);
				let new_head = head_start + head_col.min(head_line_len);
				let new_range = tome_base::range::Range::new(new_anchor, new_head);

				if new_ranges.contains(&new_range) {
					continue;
				}

				new_ranges.push(new_range);
				if idx == primary_index {
					primary_index = new_ranges.len() - 1;
				}
			}

			let sel = tome_base::selection::Selection::from_vec(new_ranges, primary_index);
			ctx.set_selection(sel.clone());
			ctx.set_cursor(sel.primary().head);
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_MERGE_SELECTIONS_HANDLERS,
	HANDLE_MERGE_SELECTIONS,
	"merge_selections",
	|r, ctx, _| {
		if matches!(r, ActionResult::MergeSelections) {
			let mut sel = ctx.selection().clone();
			sel.merge_overlaps_and_adjacent();
			ctx.set_selection(sel.clone());
			ctx.set_cursor(sel.primary().head);
		}
		HandleOutcome::Handled
	}
);
