//! Insert mode entry actions.

use evildoer_base::key::Key;
use evildoer_manifest::actions::{ActionMode, ActionResult, insert_with_motion};
use evildoer_manifest::bound_action;

bound_action!(
	insert_before,
	description: "Insert before cursor",
	bindings: [Normal => [Key::char('i')]],
	|_ctx| ActionResult::ModeChange(ActionMode::Insert)
);

bound_action!(
	insert_after,
	description: "Insert after cursor",
	bindings: [Normal => [Key::char('a')]],
	|ctx| insert_with_motion(ctx, "move_right")
);

bound_action!(
	insert_line_start,
	description: "Insert at first non-blank",
	bindings: [Normal => [Key::char('I')]],
	|ctx| insert_with_motion(ctx, "first_nonwhitespace")
);

bound_action!(
	insert_line_end,
	description: "Insert at line end",
	bindings: [Normal => [Key::char('A')]],
	|ctx| insert_with_motion(ctx, "line_end")
);
