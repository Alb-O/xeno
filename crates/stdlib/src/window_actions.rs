//! Window mode actions with colocated keybindings and handlers.
//!
//! Split names follow Vim/Helix conventions based on the divider line orientation:
//! - `split_horizontal` (Ctrl+w s): horizontal divider → windows stacked top/bottom
//! - `split_vertical` (Ctrl+w v): vertical divider → windows side-by-side left/right

use evildoer_base::key::Key;
use evildoer_manifest::action;
use evildoer_manifest::actions::{
	ActionResult, RESULT_BUFFER_NEXT_HANDLERS, RESULT_BUFFER_PREV_HANDLERS,
	RESULT_CLOSE_BUFFER_HANDLERS, RESULT_CLOSE_OTHER_BUFFERS_HANDLERS, RESULT_FOCUS_DOWN_HANDLERS,
	RESULT_FOCUS_LEFT_HANDLERS, RESULT_FOCUS_RIGHT_HANDLERS, RESULT_FOCUS_UP_HANDLERS,
	RESULT_SPLIT_HORIZONTAL_HANDLERS, RESULT_SPLIT_TERMINAL_HORIZONTAL_HANDLERS,
	RESULT_SPLIT_TERMINAL_VERTICAL_HANDLERS, RESULT_SPLIT_VERTICAL_HANDLERS,
};
use evildoer_manifest::keybindings::{BindingMode, KEYBINDINGS_WINDOW, KeyBindingDef};
use linkme::distributed_slice;

use crate::NotifyWARNExt;

const DEFAULT_PRIORITY: i16 = 100;

macro_rules! window_action {
	(
		$name:ident,
		key: $key:expr,
		description: $desc:expr,
		result: $result:ident => $slice:ident,
		handler: |$ops:ident| $body:expr
	) => {
		paste::paste! {
			action!($name, { description: $desc }, result: ActionResult::$result);

			#[distributed_slice(KEYBINDINGS_WINDOW)]
			static [<KB_ $name:upper>]: KeyBindingDef = KeyBindingDef {
				mode: BindingMode::Window,
				key: $key,
				action: stringify!($name),
				priority: DEFAULT_PRIORITY,
			};

			#[distributed_slice($slice)]
			static [<HANDLE_ $name:upper>]: evildoer_manifest::editor_ctx::ResultHandler =
				evildoer_manifest::editor_ctx::ResultHandler {
					name: stringify!($name),
					handle: |r, ctx, _| {
						if matches!(r, ActionResult::$result) {
							if let Some($ops) = ctx.buffer_ops() {
								$body;
							} else {
								ctx.warn("Buffer operations not available");
							}
						}
						evildoer_manifest::editor_ctx::HandleOutcome::Handled
					},
				};
		}
	};
}

window_action!(
	split_horizontal,
	key: Key::char('s'),
	description: "Split horizontally (new buffer below)",
	result: SplitHorizontal => RESULT_SPLIT_HORIZONTAL_HANDLERS,
	handler: |ops| ops.split_horizontal()
);

window_action!(
	split_vertical,
	key: Key::char('v'),
	description: "Split vertically (new buffer to right)",
	result: SplitVertical => RESULT_SPLIT_VERTICAL_HANDLERS,
	handler: |ops| ops.split_vertical()
);

window_action!(
	split_terminal_horizontal,
	key: Key::char('t'),
	description: "Open terminal in horizontal split (below)",
	result: SplitTerminalHorizontal => RESULT_SPLIT_TERMINAL_HORIZONTAL_HANDLERS,
	handler: |ops| ops.split_terminal_horizontal()
);

window_action!(
	split_terminal_vertical,
	key: Key::char('T'),
	description: "Open terminal in vertical split (right)",
	result: SplitTerminalVertical => RESULT_SPLIT_TERMINAL_VERTICAL_HANDLERS,
	handler: |ops| ops.split_terminal_vertical()
);

window_action!(
	focus_left,
	key: Key::char('h'),
	description: "Focus split to the left",
	result: FocusLeft => RESULT_FOCUS_LEFT_HANDLERS,
	handler: |ops| ops.focus_left()
);

window_action!(
	focus_down,
	key: Key::char('j'),
	description: "Focus split below",
	result: FocusDown => RESULT_FOCUS_DOWN_HANDLERS,
	handler: |ops| ops.focus_down()
);

window_action!(
	focus_up,
	key: Key::char('k'),
	description: "Focus split above",
	result: FocusUp => RESULT_FOCUS_UP_HANDLERS,
	handler: |ops| ops.focus_up()
);

window_action!(
	focus_right,
	key: Key::char('l'),
	description: "Focus split to the right",
	result: FocusRight => RESULT_FOCUS_RIGHT_HANDLERS,
	handler: |ops| ops.focus_right()
);

window_action!(
	buffer_next,
	key: Key::char('n'),
	description: "Switch to next buffer",
	result: BufferNext => RESULT_BUFFER_NEXT_HANDLERS,
	handler: |ops| ops.buffer_next()
);

window_action!(
	buffer_prev,
	key: Key::char('p'),
	description: "Switch to previous buffer",
	result: BufferPrev => RESULT_BUFFER_PREV_HANDLERS,
	handler: |ops| ops.buffer_prev()
);

window_action!(
	close_buffer,
	key: Key::char('q'),
	description: "Close current buffer",
	result: CloseBuffer => RESULT_CLOSE_BUFFER_HANDLERS,
	handler: |ops| ops.close_buffer()
);

window_action!(
	close_other_buffers,
	key: Key::char('o'),
	description: "Close all other buffers",
	result: CloseOtherBuffers => RESULT_CLOSE_OTHER_BUFFERS_HANDLERS,
	handler: |ops| ops.close_other_buffers()
);

#[distributed_slice(KEYBINDINGS_WINDOW)]
static KB_CLOSE_BUFFER_ALT: KeyBindingDef = KeyBindingDef {
	mode: BindingMode::Window,
	key: Key::char('c'),
	action: "close_buffer",
	priority: DEFAULT_PRIORITY,
};
