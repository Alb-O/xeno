//! Insert mode entry actions.

use evildoer_manifest::action;
use evildoer_manifest::actions::{ActionMode, ActionResult, insert_with_motion};

action!(insert_before, {
	description: "Insert before cursor",
	bindings: r#"normal "i""#,
}, |_ctx| ActionResult::ModeChange(ActionMode::Insert));

action!(insert_after, {
	description: "Insert after cursor",
	bindings: r#"normal "a""#,
}, |ctx| insert_with_motion(ctx, "move_right"));

action!(insert_line_start, {
	description: "Insert at first non-blank",
	bindings: r#"normal "I""#,
}, |ctx| insert_with_motion(ctx, "first_nonwhitespace"));

action!(insert_line_end, {
	description: "Insert at line end",
	bindings: r#"normal "A""#,
}, |ctx| insert_with_motion(ctx, "line_end"));
