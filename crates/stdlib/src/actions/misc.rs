//! Miscellaneous actions: jump list, macros, marks, redraw, and more.

use super::{ActionResult, EditAction};
use crate::action;

action!(
	jump_forward,
	{ description: "Jump forward in jump list" },
	result: ActionResult::JumpForward
);

action!(
	jump_backward,
	{ description: "Jump backward in jump list" },
	result: ActionResult::JumpBackward
);

action!(
	save_jump,
	{ description: "Save current position to jump list" },
	result: ActionResult::SaveJump
);

action!(
	record_macro,
	{ description: "Start/stop recording macro" },
	result: ActionResult::RecordMacro
);

action!(
	play_macro,
	{ description: "Play recorded macro" },
	result: ActionResult::PlayMacro
);

action!(
	save_selections,
	{ description: "Save current selections to mark" },
	result: ActionResult::SaveSelections
);

action!(
	restore_selections,
	{ description: "Restore selections from mark" },
	result: ActionResult::RestoreSelections
);

action!(
	force_redraw,
	{ description: "Force screen redraw" },
	result: ActionResult::ForceRedraw
);

action!(
	add_line_below,
	{ description: "Add empty line below cursor" },
	result: ActionResult::Edit(EditAction::AddLineBelow)
);

action!(
	add_line_above,
	{ description: "Add empty line above cursor" },
	result: ActionResult::Edit(EditAction::AddLineAbove)
);

action!(
	repeat_last_insert,
	{ description: "Repeat the last insert/change action" },
	result: ActionResult::RepeatLastInsert
);

action!(
	repeat_last_object,
	{ description: "Repeat the last object/find operation" },
	result: ActionResult::RepeatLastObject
);

action!(
	duplicate_selections_down,
	{ description: "Duplicate selections on next lines" },
	result: ActionResult::DuplicateSelectionsDown
);

action!(
	duplicate_selections_up,
	{ description: "Duplicate selections on previous lines" },
	result: ActionResult::DuplicateSelectionsUp
);

action!(
	merge_selections,
	{ description: "Merge overlapping selections" },
	result: ActionResult::MergeSelections
);

action!(
	align,
	{ description: "Align cursors" },
	result: ActionResult::Align
);

action!(
	copy_indent,
	{ description: "Copy indent from previous line" },
	result: ActionResult::CopyIndent
);

action!(
	tabs_to_spaces,
	{ description: "Convert tabs to spaces" },
	result: ActionResult::TabsToSpaces
);

action!(
	spaces_to_tabs,
	{ description: "Convert spaces to tabs" },
	result: ActionResult::SpacesToTabs
);

action!(
	trim_selections,
	{ description: "Trim whitespace from selections" },
	result: ActionResult::TrimSelections
);
