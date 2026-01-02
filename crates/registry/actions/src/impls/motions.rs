use evildoer_registry_motions::keys as motions;

use crate::{action, cursor_motion, selection_motion, ActionResult, ScreenPosition};

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

action!(next_word_start, { description: "Move to next word start", bindings: r#"normal "w""# },
	|ctx| cursor_motion(ctx, motions::next_word_start));

action!(prev_word_start, { description: "Move to previous word start", bindings: r#"normal "b""# },
	|ctx| cursor_motion(ctx, motions::prev_word_start));

action!(next_word_end, { description: "Move to next word end", bindings: r#"normal "e""# },
	|ctx| cursor_motion(ctx, motions::next_word_end));

action!(next_long_word_start, { description: "Move to next WORD start", bindings: r#"normal "W""# },
	|ctx| cursor_motion(ctx, motions::next_long_word_start));

action!(prev_long_word_start, { description: "Move to previous WORD start", bindings: r#"normal "B""# },
	|ctx| cursor_motion(ctx, motions::prev_long_word_start));

action!(next_long_word_end, { description: "Move to next WORD end", bindings: r#"normal "E""# },
	|ctx| cursor_motion(ctx, motions::next_long_word_end));

action!(select_word_forward, { description: "Select to next word start", bindings: r#"normal "alt-w""# },
	|ctx| selection_motion(ctx, motions::next_word_start));

action!(select_word_backward, { description: "Select to previous word start", bindings: r#"normal "alt-b""# },
	|ctx| selection_motion(ctx, motions::prev_word_start));

action!(select_word_end, { description: "Select to next word end", bindings: r#"normal "alt-e""# },
	|ctx| selection_motion(ctx, motions::next_word_end));

action!(document_start, { description: "Move to document start", bindings: r#"normal "g g""# },
	|ctx| cursor_motion(ctx, motions::document_start));

action!(document_end, { description: "Move to document end", bindings: r#"normal "G""# },
	|ctx| cursor_motion(ctx, motions::document_end));

action!(move_top_screen, { description: "Move to top of screen", bindings: r#"normal "H""# }, |ctx| {
	ActionResult::ScreenMotion {
		position: ScreenPosition::Top,
		count: ctx.count,
	}
});

action!(move_middle_screen, { description: "Move to middle of screen", bindings: r#"normal "M""# }, |ctx| {
	ActionResult::ScreenMotion {
		position: ScreenPosition::Middle,
		count: ctx.count,
	}
});

action!(move_bottom_screen, { description: "Move to bottom of screen" }, |ctx| {
	ActionResult::ScreenMotion {
		position: ScreenPosition::Bottom,
		count: ctx.count,
	}
});
