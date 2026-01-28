use xeno_primitives::{MotionId, motion_ids};

use crate::actions::{ActionEffects, ActionResult, ScreenPosition, action};

pub fn cursor_motion(ctx: &crate::actions::ActionContext, id: MotionId) -> ActionResult {
	ActionResult::Effects(ActionEffects::cursor_motion(id, ctx.count, ctx.extend))
}

pub fn selection_motion(ctx: &crate::actions::ActionContext, id: MotionId) -> ActionResult {
	ActionResult::Effects(ActionEffects::selection_motion(id, ctx.count, ctx.extend))
}

fn word_motion(ctx: &crate::actions::ActionContext, id: MotionId) -> ActionResult {
	ActionResult::Effects(ActionEffects::word_motion(id, ctx.count, ctx.extend))
}

action!(move_left, {
	description: "Move cursor left",
	bindings: r#"normal "h" "left"
insert "left""#,
}, |ctx| cursor_motion(ctx, motion_ids::LEFT));

action!(move_right, {
	description: "Move cursor right",
	bindings: r#"normal "l" "right"
insert "right""#,
}, |ctx| cursor_motion(ctx, motion_ids::RIGHT));

action!(move_up, {
	description: "Move cursor up",
	bindings: r#"normal "k" "up"
insert "up""#,
}, |ctx| cursor_motion(ctx, motion_ids::UP));

action!(move_down, {
	description: "Move cursor down",
	bindings: r#"normal "j" "down"
insert "down""#,
}, |ctx| cursor_motion(ctx, motion_ids::DOWN));

action!(move_line_start, { description: "Move to start of line", bindings: r#"normal "0" "home""# },
	|ctx| cursor_motion(ctx, motion_ids::LINE_START));

action!(move_line_end, { description: "Move to end of line", bindings: r#"normal "$" "end""# },
	|ctx| cursor_motion(ctx, motion_ids::LINE_END));

action!(next_word_start, {
	description: "Move to next word start",
	bindings: r#"normal "w" "ctrl-right"
insert "ctrl-right""#,
}, |ctx| word_motion(ctx, motion_ids::NEXT_WORD_START));

action!(prev_word_start, {
	description: "Move to previous word start",
	bindings: r#"normal "b" "ctrl-left"
insert "ctrl-left""#,
}, |ctx| word_motion(ctx, motion_ids::PREV_WORD_START));

action!(next_word_end, { description: "Move to next word end", bindings: r#"normal "e""# },
	|ctx| word_motion(ctx, motion_ids::NEXT_WORD_END));

action!(next_long_word_start, { description: "Move to next WORD start", bindings: r#"normal "W""# },
	|ctx| word_motion(ctx, motion_ids::NEXT_LONG_WORD_START));

action!(prev_long_word_start, { description: "Move to previous WORD start", bindings: r#"normal "B""# },
	|ctx| word_motion(ctx, motion_ids::PREV_LONG_WORD_START));

action!(next_long_word_end, { description: "Move to next WORD end", bindings: r#"normal "E""# },
	|ctx| word_motion(ctx, motion_ids::NEXT_LONG_WORD_END));

action!(select_word_forward, { description: "Select to next word start", bindings: r#"normal "alt-w""# },
	|ctx| selection_motion(ctx, motion_ids::NEXT_WORD_START));

action!(select_word_backward, { description: "Select to previous word start", bindings: r#"normal "alt-b""# },
	|ctx| selection_motion(ctx, motion_ids::PREV_WORD_START));

action!(select_word_end, { description: "Select to next word end", bindings: r#"normal "alt-e""# },
	|ctx| selection_motion(ctx, motion_ids::NEXT_WORD_END));

action!(next_paragraph, {
	description: "Move to next paragraph",
	bindings: r#"normal "}" "ctrl-down"
insert "ctrl-down""#,
}, |ctx| cursor_motion(ctx, motion_ids::NEXT_PARAGRAPH));

action!(prev_paragraph, {
	description: "Move to previous paragraph",
	bindings: r#"normal "{" "ctrl-up"
insert "ctrl-up""#,
}, |ctx| cursor_motion(ctx, motion_ids::PREV_PARAGRAPH));

action!(document_start, {
	description: "Goto file start",
	short_desc: "File start",
	bindings: r#"normal "g g""#,
}, |ctx| cursor_motion(ctx, motion_ids::DOCUMENT_START));

action!(document_end, {
	description: "Goto file end",
	short_desc: "File end",
	bindings: r#"normal "g e" "G""#,
}, |ctx| cursor_motion(ctx, motion_ids::DOCUMENT_END));

action!(goto_line_start, {
	description: "Goto line start",
	short_desc: "Line start",
	bindings: r#"normal "g h""#,
}, |ctx| cursor_motion(ctx, motion_ids::LINE_START));

action!(goto_line_end, {
	description: "Goto line end",
	short_desc: "Line end",
	bindings: r#"normal "g l""#,
}, |ctx| cursor_motion(ctx, motion_ids::LINE_END));

action!(goto_first_nonwhitespace, {
	description: "Goto first non-blank",
	short_desc: "First non-blank",
	bindings: r#"normal "g s""#,
}, |ctx| cursor_motion(ctx, motion_ids::FIRST_NONWHITESPACE));

action!(move_top_screen, { description: "Move to top of screen", bindings: r#"normal "H""# }, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Top, ctx.count))
});

action!(move_middle_screen, { description: "Move to middle of screen", bindings: r#"normal "M""# }, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Middle, ctx.count))
});

action!(move_bottom_screen, { description: "Move to bottom of screen" }, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Bottom, ctx.count))
});

action!(goto_next_hunk, {
	description: "Goto next diff hunk",
	short_desc: "Next hunk",
	bindings: r#"normal "] c""#,
}, |ctx| cursor_motion(ctx, motion_ids::NEXT_HUNK));

action!(goto_prev_hunk, {
	description: "Goto previous diff hunk",
	short_desc: "Previous hunk",
	bindings: r#"normal "[ c""#,
}, |ctx| cursor_motion(ctx, motion_ids::PREV_HUNK));

pub(super) const DEFS: &[&crate::actions::ActionDef] = &[
	&ACTION_move_left,
	&ACTION_move_right,
	&ACTION_move_up,
	&ACTION_move_down,
	&ACTION_move_line_start,
	&ACTION_move_line_end,
	&ACTION_next_word_start,
	&ACTION_prev_word_start,
	&ACTION_next_word_end,
	&ACTION_next_long_word_start,
	&ACTION_prev_long_word_start,
	&ACTION_next_long_word_end,
	&ACTION_select_word_forward,
	&ACTION_select_word_backward,
	&ACTION_select_word_end,
	&ACTION_next_paragraph,
	&ACTION_prev_paragraph,
	&ACTION_document_start,
	&ACTION_document_end,
	&ACTION_goto_line_start,
	&ACTION_goto_line_end,
	&ACTION_goto_first_nonwhitespace,
	&ACTION_move_top_screen,
	&ACTION_move_middle_screen,
	&ACTION_move_bottom_screen,
	&ACTION_goto_next_hunk,
	&ACTION_goto_prev_hunk,
];
