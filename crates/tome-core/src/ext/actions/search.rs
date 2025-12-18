//! Search-related actions.

use linkme::distributed_slice;

use crate::ext::actions::{ACTIONS, ActionDef, ActionMode, ActionResult};

#[distributed_slice(ACTIONS)]
static ACTION_SEARCH_FORWARD: ActionDef = ActionDef {
	name: "search_forward",
	description: "Search forward (enter search mode)",
	handler: |_ctx| ActionResult::ModeChange(ActionMode::SearchForward),
};

#[distributed_slice(ACTIONS)]
static ACTION_SEARCH_BACKWARD: ActionDef = ActionDef {
	name: "search_backward",
	description: "Search backward (enter search mode)",
	handler: |_ctx| ActionResult::ModeChange(ActionMode::SearchBackward),
};

#[distributed_slice(ACTIONS)]
static ACTION_SEARCH_NEXT: ActionDef = ActionDef {
	name: "search_next",
	description: "Go to next search match",
	handler: |_ctx| ActionResult::SearchNext {
		add_selection: false,
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_SEARCH_PREV: ActionDef = ActionDef {
	name: "search_prev",
	description: "Go to previous search match",
	handler: |_ctx| ActionResult::SearchPrev {
		add_selection: false,
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_SEARCH_NEXT_ADD: ActionDef = ActionDef {
	name: "search_next_add",
	description: "Add next search match to selections",
	handler: |_ctx| ActionResult::SearchNext {
		add_selection: true,
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_SEARCH_PREV_ADD: ActionDef = ActionDef {
	name: "search_prev_add",
	description: "Add previous search match to selections",
	handler: |_ctx| ActionResult::SearchPrev {
		add_selection: true,
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_USE_SELECTION_AS_SEARCH: ActionDef = ActionDef {
	name: "use_selection_as_search",
	description: "Use current selection as search pattern",
	handler: |_ctx| ActionResult::UseSelectionAsSearch,
};
