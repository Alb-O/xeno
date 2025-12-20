//! Mode-changing actions.

use crate::action;
use crate::ext::actions::{ActionContext, ActionMode, ActionResult};

action!(goto_mode, "Enter goto mode", handler: action_goto_mode);

fn action_goto_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::Goto)
}

action!(view_mode, "Enter view mode", handler: action_view_mode);

fn action_view_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::View)
}

action!(insert_mode, "Enter insert mode", handler: action_insert_mode);

fn action_insert_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::Insert)
}

action!(command_mode, "Open command scratch buffer", handler: action_command_mode);

fn action_command_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::Command)
}
