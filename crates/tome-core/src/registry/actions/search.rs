//! Search-related actions.

use crate::action;
use crate::registry::actions::{ActionMode, ActionResult};

action!(
	search_forward,
	{ description: "Search forward (enter search mode)" },
	result: ActionResult::ModeChange(ActionMode::SearchForward)
);
action!(
	search_backward,
	{ description: "Search backward (enter search mode)" },
	result: ActionResult::ModeChange(ActionMode::SearchBackward)
);
action!(
	search_next,
	{ description: "Go to next search match" },
	result: ActionResult::SearchNext {
		add_selection: false
	}
);
action!(
	search_prev,
	{ description: "Go to previous search match" },
	result: ActionResult::SearchPrev {
		add_selection: false
	}
);
action!(
	search_next_add,
	{ description: "Add next search match to selections" },
	result: ActionResult::SearchNext {
		add_selection: true
	}
);
action!(
	search_prev_add,
	{ description: "Add previous search match to selections" },
	result: ActionResult::SearchPrev {
		add_selection: true
	}
);
action!(
	use_selection_as_search,
	{ description: "Use current selection as search pattern" },
	result: ActionResult::UseSelectionAsSearch
);
