//! Search-related actions.

use crate::action;
use crate::ext::actions::{ActionMode, ActionResult};

action!(
	search_forward,
	"Search forward (enter search mode)",
	ActionResult::ModeChange(ActionMode::SearchForward)
);
action!(
	search_backward,
	"Search backward (enter search mode)",
	ActionResult::ModeChange(ActionMode::SearchBackward)
);
action!(
	search_next,
	"Go to next search match",
	ActionResult::SearchNext {
		add_selection: false
	}
);
action!(
	search_prev,
	"Go to previous search match",
	ActionResult::SearchPrev {
		add_selection: false
	}
);
action!(
	search_next_add,
	"Add next search match to selections",
	ActionResult::SearchNext {
		add_selection: true
	}
);
action!(
	search_prev_add,
	"Add previous search match to selections",
	ActionResult::SearchPrev {
		add_selection: true
	}
);
action!(
	use_selection_as_search,
	"Use current selection as search pattern",
	ActionResult::UseSelectionAsSearch
);
