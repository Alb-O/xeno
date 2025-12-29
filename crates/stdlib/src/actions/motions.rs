//! Motion actions that wrap [`MotionDef`](evildoer_manifest::MotionDef) primitives.

use evildoer_manifest::actions::{cursor_motion, selection_motion};
use evildoer_manifest::bound_action;

use crate::action;

bound_action!(move_left, description: "Move left",
	bindings: r#"normal "h" "left"
insert "left""#,
	|ctx| cursor_motion(ctx, "move_left"));

bound_action!(move_right, description: "Move right",
	bindings: r#"normal "l" "right"
insert "right""#,
	|ctx| cursor_motion(ctx, "move_right"));

action!(move_up, { description: "Move up" }, |ctx| cursor_motion(ctx, "move_up"));
action!(move_down, { description: "Move down" }, |ctx| cursor_motion(ctx, "move_down"));

bound_action!(move_line_start, description: "Move to line start",
	bindings: r#"normal "0" "home" "alt-h"
goto "h"
insert "home""#,
	|ctx| cursor_motion(ctx, "line_start"));

bound_action!(move_line_end, description: "Move to line end",
	bindings: r#"normal "$" "end" "alt-l"
goto "l"
insert "end""#,
	|ctx| cursor_motion(ctx, "line_end"));

bound_action!(move_first_nonblank, description: "Move to first non-blank",
	bindings: r#"normal "^"
goto "i""#,
	|ctx| cursor_motion(ctx, "first_nonwhitespace"));

bound_action!(document_start, description: "Move to document start",
	bindings: r#"normal "ctrl-home"
goto "g" "k"
insert "ctrl-home""#,
	|ctx| cursor_motion(ctx, "document_start"));

bound_action!(document_end, description: "Move to document end",
	bindings: r#"normal "G" "ctrl-end"
goto "j" "e"
insert "ctrl-end""#,
	|ctx| cursor_motion(ctx, "document_end"));

bound_action!(next_word_start, description: "Move to next word start",
	bindings: r#"normal "w"
insert "ctrl-right""#,
	|ctx| selection_motion(ctx, "next_word_start"));

bound_action!(next_word_end, description: "Move to next word end",
	bindings: r#"normal "e""#,
	|ctx| selection_motion(ctx, "next_word_end"));

bound_action!(prev_word_start, description: "Move to previous word start",
	bindings: r#"normal "b"
insert "ctrl-left""#,
	|ctx| selection_motion(ctx, "prev_word_start"));

bound_action!(prev_word_end, description: "Move to previous word end",
	bindings: r#"normal "alt-e""#,
	|ctx| selection_motion(ctx, "prev_word_end"));

bound_action!(next_long_word_start, description: "Move to next WORD start",
	bindings: r#"normal "W" "alt-w""#,
	|ctx| selection_motion(ctx, "next_long_word_start"));

bound_action!(next_long_word_end, description: "Move to next WORD end",
	bindings: r#"normal "E""#,
	|ctx| selection_motion(ctx, "next_long_word_end"));

bound_action!(prev_long_word_start, description: "Move to previous WORD start",
	bindings: r#"normal "B" "alt-b""#,
	|ctx| selection_motion(ctx, "prev_long_word_start"));
