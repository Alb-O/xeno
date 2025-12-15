//! Miscellaneous actions: jump list, macros, marks, redraw, and more.

use linkme::distributed_slice;

use super::{ActionContext, ActionDef, ActionResult, EditAction, ACTIONS};

macro_rules! action {
    ($name:ident, $id:expr, $desc:expr, $handler:expr) => {
        #[distributed_slice(ACTIONS)]
        static $name: ActionDef = ActionDef {
            name: $id,
            description: $desc,
            handler: $handler,
        };
    };
}

// Jump list
action!(
    ACTION_JUMP_FORWARD,
    "jump_forward",
    "Jump forward in jump list",
    |_ctx: &ActionContext| ActionResult::JumpForward
);

action!(
    ACTION_JUMP_BACKWARD,
    "jump_backward",
    "Jump backward in jump list",
    |_ctx: &ActionContext| ActionResult::JumpBackward
);

action!(
    ACTION_SAVE_JUMP,
    "save_jump",
    "Save current position to jump list",
    |_ctx: &ActionContext| ActionResult::SaveJump
);

// Macros
action!(
    ACTION_RECORD_MACRO,
    "record_macro",
    "Start/stop recording macro",
    |_ctx: &ActionContext| ActionResult::RecordMacro
);

action!(
    ACTION_PLAY_MACRO,
    "play_macro",
    "Play recorded macro",
    |_ctx: &ActionContext| ActionResult::PlayMacro
);

// Marks/Selections
action!(
    ACTION_SAVE_SELECTIONS,
    "save_selections",
    "Save current selections to mark",
    |_ctx: &ActionContext| ActionResult::SaveSelections
);

action!(
    ACTION_RESTORE_SELECTIONS,
    "restore_selections",
    "Restore selections from mark",
    |_ctx: &ActionContext| ActionResult::RestoreSelections
);

// Redraw
action!(
    ACTION_FORCE_REDRAW,
    "force_redraw",
    "Force screen redraw",
    |_ctx: &ActionContext| ActionResult::ForceRedraw
);

// Add empty lines
action!(
    ACTION_ADD_LINE_BELOW,
    "add_line_below",
    "Add empty line below cursor",
    |_ctx: &ActionContext| ActionResult::Edit(EditAction::AddLineBelow)
);

action!(
    ACTION_ADD_LINE_ABOVE,
    "add_line_above",
    "Add empty line above cursor",
    |_ctx: &ActionContext| ActionResult::Edit(EditAction::AddLineAbove)
);
