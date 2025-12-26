//! Mode-changing actions.

use tome_manifest::actions::{ActionContext, ActionMode, ActionResult};

use crate::action;

action!(goto_mode, { description: "Enter goto mode" }, handler: action_goto_mode);

fn action_goto_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::Goto)
}

action!(view_mode, { description: "Enter view mode" }, handler: action_view_mode);

fn action_view_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::View)
}

action!(insert_mode, { description: "Enter insert mode" }, handler: action_insert_mode);

fn action_insert_mode(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::Insert)
}
