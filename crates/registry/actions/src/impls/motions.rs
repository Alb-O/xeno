use evildoer_registry_motions::{find as find_motion, keys as motions, MotionKey};

use crate::{action, cursor_motion, selection_motion, ActionResult};

/// Lookup motions that are defined at runtime (no static keys yet).
fn runtime_motion(name: &str) -> Option<MotionKey> {
	find_motion(name)
}

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

action!(move_up, { description: "Move cursor up" }, |ctx| {
	cursor_motion(ctx, motions::up)
});

action!(move_down, { description: "Move cursor down" }, |ctx| {
	cursor_motion(ctx, motions::down)
});

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
	let Some(motion) = runtime_motion("screen_top") else {
		return ActionResult::Error("Unknown motion: screen_top".to_string());
	};
	cursor_motion(ctx, motion)
});

action!(move_middle_screen, { description: "Move to middle of screen", bindings: r#"normal "M""# }, |ctx| {
	let Some(motion) = runtime_motion("screen_middle") else {
		return ActionResult::Error("Unknown motion: screen_middle".to_string());
	};
	cursor_motion(ctx, motion)
});

action!(move_bottom_screen, { description: "Move to bottom of screen" }, |ctx| {
	let Some(motion) = runtime_motion("screen_bottom") else {
		return ActionResult::Error("Unknown motion: screen_bottom".to_string());
	};
	cursor_motion(ctx, motion)
});
