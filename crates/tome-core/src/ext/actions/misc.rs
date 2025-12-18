//! Miscellaneous actions: jump list, macros, marks, redraw, and more.

use linkme::distributed_slice;

use super::{ACTIONS, ActionContext, ActionDef, ActionResult, EditAction};

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

action!(
	ACTION_FORCE_REDRAW,
	"force_redraw",
	"Force screen redraw",
	|_ctx: &ActionContext| ActionResult::ForceRedraw
);

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

action!(
	ACTION_REPEAT_LAST_INSERT,
	"repeat_last_insert",
	"Repeat the last insert/change action",
	|_ctx: &ActionContext| ActionResult::RepeatLastInsert
);

action!(
	ACTION_REPEAT_LAST_OBJECT,
	"repeat_last_object",
	"Repeat the last object/find operation",
	|_ctx: &ActionContext| ActionResult::RepeatLastObject
);

action!(
	ACTION_DUPLICATE_DOWN,
	"duplicate_selections_down",
	"Duplicate selections on next lines",
	|_ctx: &ActionContext| ActionResult::DuplicateSelectionsDown
);

action!(
	ACTION_DUPLICATE_UP,
	"duplicate_selections_up",
	"Duplicate selections on previous lines",
	|_ctx: &ActionContext| ActionResult::DuplicateSelectionsUp
);

action!(
	ACTION_MERGE_SELECTIONS,
	"merge_selections",
	"Merge overlapping selections",
	|_ctx: &ActionContext| ActionResult::MergeSelections
);

action!(
	ACTION_ALIGN,
	"align",
	"Align cursors",
	|_ctx: &ActionContext| ActionResult::Align
);

action!(
	ACTION_COPY_INDENT,
	"copy_indent",
	"Copy indent from previous line",
	|_ctx: &ActionContext| ActionResult::CopyIndent
);

action!(
	ACTION_TABS_TO_SPACES,
	"tabs_to_spaces",
	"Convert tabs to spaces",
	|_ctx: &ActionContext| ActionResult::TabsToSpaces
);

action!(
	ACTION_SPACES_TO_TABS,
	"spaces_to_tabs",
	"Convert spaces to tabs",
	|_ctx: &ActionContext| ActionResult::SpacesToTabs
);

action!(
	ACTION_TRIM_SELECTIONS,
	"trim_selections",
	"Trim whitespace from selections",
	|_ctx: &ActionContext| ActionResult::TrimSelections
);

action!(
	ACTION_OPEN_SCRATCH,
	"open_scratch",
	"Open command scratch buffer",
	|_ctx: &ActionContext| ActionResult::OpenScratch { focus: true }
);

action!(
	ACTION_CLOSE_SCRATCH,
	"close_scratch",
	"Close command scratch buffer",
	|_ctx: &ActionContext| ActionResult::CloseScratch
);

action!(
	ACTION_TOGGLE_SCRATCH,
	"toggle_scratch",
	"Toggle command scratch buffer",
	|_ctx: &ActionContext| ActionResult::ToggleScratch
);

action!(
	ACTION_EXECUTE_SCRATCH,
	"execute_scratch",
	"Execute scratch buffer contents",
	|_ctx: &ActionContext| ActionResult::ExecuteScratch
);
