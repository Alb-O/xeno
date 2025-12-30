//! Window mode actions with colocated keybindings and handlers.
//!
//! Split names follow Vim/Helix conventions based on the divider line orientation:
//! - `split_horizontal` (Ctrl+w s): horizontal divider → windows stacked top/bottom
//! - `split_vertical` (Ctrl+w v): vertical divider → windows side-by-side left/right

use evildoer_manifest::{action, result_handler};

action!(split_horizontal, {
	description: "Split horizontally (new buffer below)",
	bindings: r#"window "s""#,
	result: SplitHorizontal,
}, |ops| ops.split_horizontal());

action!(split_vertical, {
	description: "Split vertically (new buffer to right)",
	bindings: r#"window "v""#,
	result: SplitVertical,
}, |ops| ops.split_vertical());

action!(split_terminal_horizontal, {
	description: "Open terminal in horizontal split (below)",
	bindings: r#"window "t""#,
	result: SplitTerminalHorizontal,
}, |ops| ops.split_terminal_horizontal());

action!(split_terminal_vertical, {
	description: "Open terminal in vertical split (right)",
	bindings: r#"window "T""#,
	result: SplitTerminalVertical,
}, |ops| ops.split_terminal_vertical());

action!(toggle_terminal, {
	description: "Toggle terminal split",
	bindings: r#"normal ":""#,
}, |_ctx| evildoer_manifest::actions::ActionResult::TogglePanel("terminal"));

action!(toggle_debug_panel, {
	description: "Toggle debug panel",
	bindings: r#"normal "D""#,
}, |_ctx| evildoer_manifest::actions::ActionResult::TogglePanel("debug"));

result_handler!(
	RESULT_TOGGLE_PANEL_HANDLERS,
	TOGGLE_PANEL_HANDLER,
	"toggle_panel",
	|result, ctx, _extend| {
		use evildoer_manifest::actions::ActionResult;
		use evildoer_manifest::editor_ctx::HandleOutcome;

		let ActionResult::TogglePanel(name) = result else {
			return HandleOutcome::NotHandled;
		};

		if let Some(ops) = ctx.buffer_ops() {
			ops.toggle_panel(name);
		}
		HandleOutcome::Handled
	}
);

action!(focus_left, {
	description: "Focus split to the left",
	bindings: r#"window "h""#,
	result: FocusLeft,
}, |ops| ops.focus_left());

action!(focus_down, {
	description: "Focus split below",
	bindings: r#"window "j""#,
	result: FocusDown,
}, |ops| ops.focus_down());

action!(focus_up, {
	description: "Focus split above",
	bindings: r#"window "k""#,
	result: FocusUp,
}, |ops| ops.focus_up());

action!(focus_right, {
	description: "Focus split to the right",
	bindings: r#"window "l""#,
	result: FocusRight,
}, |ops| ops.focus_right());

action!(buffer_next, {
	description: "Switch to next buffer",
	bindings: r#"window "n""#,
	result: BufferNext,
}, |ops| ops.buffer_next());

action!(buffer_prev, {
	description: "Switch to previous buffer",
	bindings: r#"window "p""#,
	result: BufferPrev,
}, |ops| ops.buffer_prev());

action!(close_split, {
	description: "Close current split",
	bindings: r#"window "q" "c""#,
	result: CloseSplit,
}, |ops| ops.close_split());

action!(close_other_buffers, {
	description: "Close all other buffers",
	bindings: r#"window "o""#,
	result: CloseOtherBuffers,
}, |ops| ops.close_other_buffers());
