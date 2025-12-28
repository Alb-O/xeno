//! Motion actions that wrap [`MotionDef`](tome_manifest::MotionDef) primitives.

use tome_base::key::{Key, SpecialKey};
use tome_manifest::actions::{cursor_motion, selection_motion};
use tome_manifest::bound_action;

use crate::action;

bound_action!(
	move_left,
	description: "Move left",
	bindings: [
		Normal => [Key::char('h'), Key::special(SpecialKey::Left)],
		Insert => [Key::special(SpecialKey::Left)],
	],
	|ctx| cursor_motion(ctx, "move_left")
);

bound_action!(
	move_right,
	description: "Move right",
	bindings: [
		Normal => [Key::char('l'), Key::special(SpecialKey::Right)],
		Insert => [Key::special(SpecialKey::Right)],
	],
	|ctx| cursor_motion(ctx, "move_right")
);

action!(move_up, { description: "Move up" }, |ctx| cursor_motion(ctx, "move_up"));
action!(move_down, { description: "Move down" }, |ctx| cursor_motion(ctx, "move_down"));

bound_action!(
	move_line_start,
	description: "Move to line start",
	bindings: [
		Normal => [Key::char('0'), Key::special(SpecialKey::Home), Key::alt('h')],
		Goto => [Key::char('h')],
		Insert => [Key::special(SpecialKey::Home)],
	],
	|ctx| cursor_motion(ctx, "line_start")
);

bound_action!(
	move_line_end,
	description: "Move to line end",
	bindings: [
		Normal => [Key::char('$'), Key::special(SpecialKey::End), Key::alt('l')],
		Goto => [Key::char('l')],
		Insert => [Key::special(SpecialKey::End)],
	],
	|ctx| cursor_motion(ctx, "line_end")
);

bound_action!(
	move_first_nonblank,
	description: "Move to first non-blank",
	bindings: [
		Normal => [Key::char('^')],
		Goto => [Key::char('i')],
	],
	|ctx| cursor_motion(ctx, "first_nonwhitespace")
);

bound_action!(
	document_start,
	description: "Move to document start",
	bindings: [
		Normal => [Key::special(SpecialKey::Home).with_ctrl()],
		Goto => [Key::char('g'), Key::char('k')],
		Insert => [Key::special(SpecialKey::Home).with_ctrl()],
	],
	|ctx| cursor_motion(ctx, "document_start")
);

bound_action!(
	document_end,
	description: "Move to document end",
	bindings: [
		Normal => [Key::char('G'), Key::special(SpecialKey::End).with_ctrl()],
		Goto => [Key::char('j'), Key::char('e')],
		Insert => [Key::special(SpecialKey::End).with_ctrl()],
	],
	|ctx| cursor_motion(ctx, "document_end")
);

// Selection-creating motions: create selections from cursor to new position

bound_action!(
	next_word_start,
	description: "Move to next word start",
	bindings: [
		Normal => [Key::char('w')],
		Insert => [Key::special(SpecialKey::Right).with_ctrl()],
	],
	|ctx| selection_motion(ctx, "next_word_start")
);

bound_action!(
	next_word_end,
	description: "Move to next word end",
	bindings: [Normal => [Key::char('e')]],
	|ctx| selection_motion(ctx, "next_word_end")
);

bound_action!(
	prev_word_start,
	description: "Move to previous word start",
	bindings: [
		Normal => [Key::char('b')],
		Insert => [Key::special(SpecialKey::Left).with_ctrl()],
	],
	|ctx| selection_motion(ctx, "prev_word_start")
);

bound_action!(
	prev_word_end,
	description: "Move to previous word end",
	bindings: [Normal => [Key::alt('e')]],
	|ctx| selection_motion(ctx, "prev_word_end")
);

bound_action!(
	next_long_word_start,
	description: "Move to next WORD start",
	bindings: [Normal => [Key::char('W'), Key::alt('w')]],
	|ctx| selection_motion(ctx, "next_long_word_start")
);

bound_action!(
	next_long_word_end,
	description: "Move to next WORD end",
	bindings: [Normal => [Key::char('E')]],
	|ctx| selection_motion(ctx, "next_long_word_end")
);

bound_action!(
	prev_long_word_start,
	description: "Move to previous WORD start",
	bindings: [Normal => [Key::char('B'), Key::alt('b')]],
	|ctx| selection_motion(ctx, "prev_long_word_start")
);
