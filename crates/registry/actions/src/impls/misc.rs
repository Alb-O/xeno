//! Miscellaneous actions.

use crate::{ActionResult, EditAction, action};

action!(add_line_below, { description: "Add empty line below cursor" },
	|_ctx| ActionResult::Edit(EditAction::AddLineBelow));

action!(add_line_above, { description: "Add empty line above cursor" },
	|_ctx| ActionResult::Edit(EditAction::AddLineAbove));

action!(use_selection_as_search, { description: "Use current selection as search pattern" },
	|_ctx| ActionResult::UseSelectionAsSearch);
