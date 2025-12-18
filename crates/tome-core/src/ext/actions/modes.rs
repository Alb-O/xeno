//! Mode-changing actions.

use linkme::distributed_slice;

use crate::ext::actions::{ACTIONS, ActionContext, ActionDef, ActionMode, ActionResult};

fn action_goto_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::Goto)
}

#[distributed_slice(ACTIONS)]
static ACTION_GOTO_MODE: ActionDef = ActionDef {
	name: "goto_mode",
	description: "Enter goto mode",
	handler: action_goto_mode,
};

fn action_view_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::View)
}

#[distributed_slice(ACTIONS)]
static ACTION_VIEW_MODE: ActionDef = ActionDef {
	name: "view_mode",
	description: "Enter view mode",
	handler: action_view_mode,
};

fn action_insert_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::Insert)
}

#[distributed_slice(ACTIONS)]
static ACTION_INSERT_MODE: ActionDef = ActionDef {
	name: "insert_mode",
	description: "Enter insert mode",
	handler: action_insert_mode,
};

fn action_command_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ToggleScratch
}

#[distributed_slice(ACTIONS)]
static ACTION_COMMAND_MODE: ActionDef = ActionDef {
	name: "command_mode",
	description: "Open command scratch buffer",
	handler: action_command_mode,
};
