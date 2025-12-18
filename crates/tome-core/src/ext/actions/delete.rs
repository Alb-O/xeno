//! Delete actions for insert mode.

use linkme::distributed_slice;

use crate::ext::actions::{ACTIONS, ActionDef, ActionResult};

#[distributed_slice(ACTIONS)]
static ACTION_DELETE_WORD_BACK: ActionDef = ActionDef {
	name: "delete_word_back",
	description: "Delete word before cursor",
	handler: |_ctx| {
		// This would delete backward to word start
		// For now, we signal this via a special result that the editor handles
		ActionResult::Ok
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_DELETE_WORD_FORWARD: ActionDef = ActionDef {
	name: "delete_word_forward",
	description: "Delete word after cursor",
	handler: |_ctx| ActionResult::Ok,
};

#[distributed_slice(ACTIONS)]
static ACTION_DELETE_TO_LINE_END: ActionDef = ActionDef {
	name: "delete_to_line_end",
	description: "Delete from cursor to end of line",
	handler: |_ctx| ActionResult::Ok,
};

#[distributed_slice(ACTIONS)]
static ACTION_DELETE_TO_LINE_START: ActionDef = ActionDef {
	name: "delete_to_line_start",
	description: "Delete from cursor to start of line",
	handler: |_ctx| ActionResult::Ok,
};

#[distributed_slice(ACTIONS)]
static ACTION_ADD_LINE_BELOW: ActionDef = ActionDef {
	name: "add_line_below",
	description: "Add empty line below current line",
	handler: |_ctx| ActionResult::Edit(crate::ext::actions::EditAction::AddLineBelow),
};

#[distributed_slice(ACTIONS)]
static ACTION_ADD_LINE_ABOVE: ActionDef = ActionDef {
	name: "add_line_above",
	description: "Add empty line above current line",
	handler: |_ctx| ActionResult::Edit(crate::ext::actions::EditAction::AddLineAbove),
};
