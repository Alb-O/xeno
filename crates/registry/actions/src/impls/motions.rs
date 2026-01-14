use xeno_registry_motions::keys as motions;

use crate::{
	ActionEffects, ActionResult, ScreenPosition, action, cursor_motion, selection_motion,
	word_motion,
};

action!(move_left, {
	description: "Move cursor left",
	bindings: r#"normal "h" "left"
insert "left""#,
}, |ctx| cursor_motion(ctx, motions::left));

action!(move_right, {
	description: "Move cursor right",
	bindings: r#"normal "l" "right"
insert "right""#,
}, |ctx| cursor_motion(ctx, motions::right));

action!(move_line_start, { description: "Move to start of line", bindings: r#"normal "0" "home""# },
	|ctx| cursor_motion(ctx, motions::line_start));

action!(move_line_end, { description: "Move to end of line", bindings: r#"normal "$" "end""# },
	|ctx| cursor_motion(ctx, motions::line_end));

action!(next_word_start, {
	description: "Move to next word start",
	bindings: r#"normal "w" "ctrl-right"
insert "ctrl-right""#,
}, |ctx| word_motion(ctx, motions::next_word_start));

action!(prev_word_start, {
	description: "Move to previous word start",
	bindings: r#"normal "b" "ctrl-left"
insert "ctrl-left""#,
}, |ctx| word_motion(ctx, motions::prev_word_start));

action!(next_word_end, { description: "Move to next word end", bindings: r#"normal "e""# },
	|ctx| word_motion(ctx, motions::next_word_end));

action!(next_long_word_start, { description: "Move to next WORD start", bindings: r#"normal "W""# },
	|ctx| word_motion(ctx, motions::next_long_word_start));

action!(prev_long_word_start, { description: "Move to previous WORD start", bindings: r#"normal "B""# },
	|ctx| word_motion(ctx, motions::prev_long_word_start));

action!(next_long_word_end, { description: "Move to next WORD end", bindings: r#"normal "E""# },
	|ctx| word_motion(ctx, motions::next_long_word_end));

action!(select_word_forward, { description: "Select to next word start", bindings: r#"normal "alt-w""# },
	|ctx| selection_motion(ctx, motions::next_word_start));

action!(select_word_backward, { description: "Select to previous word start", bindings: r#"normal "alt-b""# },
	|ctx| selection_motion(ctx, motions::prev_word_start));

action!(select_word_end, { description: "Select to next word end", bindings: r#"normal "alt-e""# },
	|ctx| selection_motion(ctx, motions::next_word_end));

action!(document_start, {
	description: "Goto file start",
	short_desc: "File start",
	bindings: r#"normal "g g""#,
}, |ctx| cursor_motion(ctx, motions::document_start));

action!(document_end, {
	description: "Goto file end",
	short_desc: "File end",
	bindings: r#"normal "g e" "G""#,
}, |ctx| cursor_motion(ctx, motions::document_end));

action!(goto_line_start, {
	description: "Goto line start",
	short_desc: "Line start",
	bindings: r#"normal "g h""#,
}, |ctx| cursor_motion(ctx, motions::line_start));

action!(goto_line_end, {
	description: "Goto line end",
	short_desc: "Line end",
	bindings: r#"normal "g l""#,
}, |ctx| cursor_motion(ctx, motions::line_end));

action!(goto_first_nonwhitespace, {
	description: "Goto first non-blank",
	short_desc: "First non-blank",
	bindings: r#"normal "g s""#,
}, |ctx| cursor_motion(ctx, motions::first_nonwhitespace));

action!(move_top_screen, { description: "Move to top of screen", bindings: r#"normal "H""# }, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Top, ctx.count))
});

action!(move_middle_screen, { description: "Move to middle of screen", bindings: r#"normal "M""# }, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Middle, ctx.count))
});

action!(move_bottom_screen, { description: "Move to bottom of screen" }, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Bottom, ctx.count))
});
