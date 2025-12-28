//! Miscellaneous actions: jump list, macros, marks, redraw, and more.

use super::{ActionResult, EditAction};
use crate::action;

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

action!(
	use_selection_as_search,
	{ description: "Use current selection as search pattern" },
	result: ActionResult::UseSelectionAsSearch
);
