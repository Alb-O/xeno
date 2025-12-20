//! Regex-based selection manipulation actions.

use crate::action;
use crate::ext::actions::{ActionMode, ActionResult};

action!(
	select_regex,
	"Select regex matches within selection",
	ActionResult::ModeChange(ActionMode::SelectRegex)
);
action!(
	split_regex,
	"Split selection on regex matches",
	ActionResult::ModeChange(ActionMode::SplitRegex)
);
action!(
	split_lines,
	"Split selection into lines",
	ActionResult::SplitLines
);
action!(
	keep_matching,
	"Keep selections matching regex",
	ActionResult::ModeChange(ActionMode::KeepMatching)
);
action!(
	keep_not_matching,
	"Keep selections not matching regex",
	ActionResult::ModeChange(ActionMode::KeepNotMatching)
);
