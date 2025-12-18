//! Handlers for not-yet-implemented features.
//!
//! These display a message to the user indicating the feature isn't available yet.

use crate::ext::actions::ActionResult;
use crate::ext::editor_ctx::HandleOutcome;
use crate::result_handler;

macro_rules! unimplemented_handler {
	($slice:ident, $static_name:ident, $name:literal, $variant:pat, $msg:literal) => {
		result_handler!($slice, $static_name, $name, |r, ctx, _| {
			if matches!(r, $variant) {
				ctx.message($msg);
			}
			HandleOutcome::Handled
		});
	};
}

// SplitLines is handled via SelectionOpsAccess
result_handler!(
	RESULT_SPLIT_LINES_HANDLERS,
	HANDLE_SPLIT_LINES,
	"split_lines",
	|_, ctx, _| {
		if let Some(ops) = ctx.selection_ops() {
			ops.split_lines();
			HandleOutcome::Handled
		} else {
			ctx.message("Split lines not available");
			HandleOutcome::Handled
		}
	}
);
unimplemented_handler!(
	RESULT_JUMP_FORWARD_HANDLERS,
	HANDLE_JUMP_FORWARD,
	"jump_forward",
	ActionResult::JumpForward,
	"Jump list not yet implemented"
);
unimplemented_handler!(
	RESULT_JUMP_BACKWARD_HANDLERS,
	HANDLE_JUMP_BACKWARD,
	"jump_backward",
	ActionResult::JumpBackward,
	"Jump list not yet implemented"
);
unimplemented_handler!(
	RESULT_SAVE_JUMP_HANDLERS,
	HANDLE_SAVE_JUMP,
	"save_jump",
	ActionResult::SaveJump,
	"Jump list not yet implemented"
);
unimplemented_handler!(
	RESULT_RECORD_MACRO_HANDLERS,
	HANDLE_RECORD_MACRO,
	"record_macro",
	ActionResult::RecordMacro,
	"Macros not yet implemented"
);
unimplemented_handler!(
	RESULT_PLAY_MACRO_HANDLERS,
	HANDLE_PLAY_MACRO,
	"play_macro",
	ActionResult::PlayMacro,
	"Macros not yet implemented"
);
unimplemented_handler!(
	RESULT_SAVE_SELECTIONS_HANDLERS,
	HANDLE_SAVE_SELECTIONS,
	"save_selections",
	ActionResult::SaveSelections,
	"Marks not yet implemented"
);
unimplemented_handler!(
	RESULT_RESTORE_SELECTIONS_HANDLERS,
	HANDLE_RESTORE_SELECTIONS,
	"restore_selections",
	ActionResult::RestoreSelections,
	"Marks not yet implemented"
);
unimplemented_handler!(
	RESULT_REPEAT_LAST_INSERT_HANDLERS,
	HANDLE_REPEAT_LAST_INSERT,
	"repeat_last_insert",
	ActionResult::RepeatLastInsert,
	"Repeat insert not yet implemented"
);
unimplemented_handler!(
	RESULT_REPEAT_LAST_OBJECT_HANDLERS,
	HANDLE_REPEAT_LAST_OBJECT,
	"repeat_last_object",
	ActionResult::RepeatLastObject,
	"Repeat object not yet implemented"
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
				let new_range = crate::range::Range::new(new_anchor, new_head);

				if new_ranges.contains(&new_range) {
					continue;
				}

				new_ranges.push(new_range);
				if idx == primary_index {
					primary_index = new_ranges.len() - 1;
				}
			}

			let sel = crate::selection::Selection::from_vec(new_ranges, primary_index);
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
				let new_range = crate::range::Range::new(new_anchor, new_head);

				if new_ranges.contains(&new_range) {
					continue;
				}

				new_ranges.push(new_range);
				if idx == primary_index {
					primary_index = new_ranges.len() - 1;
				}
			}

			let sel = crate::selection::Selection::from_vec(new_ranges, primary_index);
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

unimplemented_handler!(
	RESULT_ALIGN_HANDLERS,
	HANDLE_ALIGN,
	"align",
	ActionResult::Align,
	"Align not yet implemented"
);
unimplemented_handler!(
	RESULT_COPY_INDENT_HANDLERS,
	HANDLE_COPY_INDENT,
	"copy_indent",
	ActionResult::CopyIndent,
	"Copy indent not yet implemented"
);
unimplemented_handler!(
	RESULT_TABS_TO_SPACES_HANDLERS,
	HANDLE_TABS_TO_SPACES,
	"tabs_to_spaces",
	ActionResult::TabsToSpaces,
	"Tabs to spaces not yet implemented"
);
unimplemented_handler!(
	RESULT_SPACES_TO_TABS_HANDLERS,
	HANDLE_SPACES_TO_TABS,
	"spaces_to_tabs",
	ActionResult::SpacesToTabs,
	"Spaces to tabs not yet implemented"
);
unimplemented_handler!(
	RESULT_TRIM_SELECTIONS_HANDLERS,
	HANDLE_TRIM_SELECTIONS,
	"trim_selections",
	ActionResult::TrimSelections,
	"Trim selections not yet implemented"
);
