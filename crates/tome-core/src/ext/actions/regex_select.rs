//! Regex-based selection manipulation actions.

use linkme::distributed_slice;

use crate::ext::actions::{ACTIONS, ActionDef, ActionMode, ActionResult};

#[distributed_slice(ACTIONS)]
static ACTION_SELECT_REGEX: ActionDef = ActionDef {
	name: "select_regex",
	description: "Select regex matches within selection",
	handler: |_ctx| ActionResult::ModeChange(ActionMode::SelectRegex),
};

#[distributed_slice(ACTIONS)]
static ACTION_SPLIT_REGEX: ActionDef = ActionDef {
	name: "split_regex",
	description: "Split selection on regex matches",
	handler: |_ctx| ActionResult::ModeChange(ActionMode::SplitRegex),
};

#[distributed_slice(ACTIONS)]
static ACTION_SPLIT_LINES: ActionDef = ActionDef {
	name: "split_lines",
	description: "Split selection into lines",
	handler: |_ctx| ActionResult::SplitLines,
};

#[distributed_slice(ACTIONS)]
static ACTION_KEEP_MATCHING: ActionDef = ActionDef {
	name: "keep_matching",
	description: "Keep selections matching regex",
	handler: |_ctx| ActionResult::ModeChange(ActionMode::KeepMatching),
};

#[distributed_slice(ACTIONS)]
static ACTION_KEEP_NOT_MATCHING: ActionDef = ActionDef {
	name: "keep_not_matching",
	description: "Keep selections not matching regex",
	handler: |_ctx| ActionResult::ModeChange(ActionMode::KeepNotMatching),
};
