//! Window mode actions with colocated keybindings and handlers.
//!
//! Split names follow Vim/Helix conventions based on the divider line orientation:
//! - `split_horizontal` (Ctrl+w s): horizontal divider → windows stacked top/bottom
//! - `split_vertical` (Ctrl+w v): vertical divider → windows side-by-side left/right

use evildoer_base::key::Key;
use evildoer_manifest::full_action;
use evildoer_manifest::keybindings::{KEYBINDINGS_WINDOW, KeyBindingDef};
use linkme::distributed_slice;

full_action!(
	split_horizontal,
	description: "Split horizontally (new buffer below)",
	key: Key::char('s'),
	mode: Window,
	result: SplitHorizontal,
	handler_slice: RESULT_SPLIT_HORIZONTAL_HANDLERS,
	|ops| ops.split_horizontal()
);

full_action!(
	split_vertical,
	description: "Split vertically (new buffer to right)",
	key: Key::char('v'),
	mode: Window,
	result: SplitVertical,
	handler_slice: RESULT_SPLIT_VERTICAL_HANDLERS,
	|ops| ops.split_vertical()
);

full_action!(
	split_terminal_horizontal,
	description: "Open terminal in horizontal split (below)",
	key: Key::char('t'),
	mode: Window,
	result: SplitTerminalHorizontal,
	handler_slice: RESULT_SPLIT_TERMINAL_HORIZONTAL_HANDLERS,
	|ops| ops.split_terminal_horizontal()
);

full_action!(
	split_terminal_vertical,
	description: "Open terminal in vertical split (right)",
	key: Key::char('T'),
	mode: Window,
	result: SplitTerminalVertical,
	handler_slice: RESULT_SPLIT_TERMINAL_VERTICAL_HANDLERS,
	|ops| ops.split_terminal_vertical()
);

full_action!(
	focus_left,
	description: "Focus split to the left",
	key: Key::char('h'),
	mode: Window,
	result: FocusLeft,
	handler_slice: RESULT_FOCUS_LEFT_HANDLERS,
	|ops| ops.focus_left()
);

full_action!(
	focus_down,
	description: "Focus split below",
	key: Key::char('j'),
	mode: Window,
	result: FocusDown,
	handler_slice: RESULT_FOCUS_DOWN_HANDLERS,
	|ops| ops.focus_down()
);

full_action!(
	focus_up,
	description: "Focus split above",
	key: Key::char('k'),
	mode: Window,
	result: FocusUp,
	handler_slice: RESULT_FOCUS_UP_HANDLERS,
	|ops| ops.focus_up()
);

full_action!(
	focus_right,
	description: "Focus split to the right",
	key: Key::char('l'),
	mode: Window,
	result: FocusRight,
	handler_slice: RESULT_FOCUS_RIGHT_HANDLERS,
	|ops| ops.focus_right()
);

full_action!(
	buffer_next,
	description: "Switch to next buffer",
	key: Key::char('n'),
	mode: Window,
	result: BufferNext,
	handler_slice: RESULT_BUFFER_NEXT_HANDLERS,
	|ops| ops.buffer_next()
);

full_action!(
	buffer_prev,
	description: "Switch to previous buffer",
	key: Key::char('p'),
	mode: Window,
	result: BufferPrev,
	handler_slice: RESULT_BUFFER_PREV_HANDLERS,
	|ops| ops.buffer_prev()
);

full_action!(
	close_buffer,
	description: "Close current buffer",
	key: Key::char('q'),
	mode: Window,
	result: CloseBuffer,
	handler_slice: RESULT_CLOSE_BUFFER_HANDLERS,
	|ops| ops.close_buffer()
);

full_action!(
	close_other_buffers,
	description: "Close all other buffers",
	key: Key::char('o'),
	mode: Window,
	result: CloseOtherBuffers,
	handler_slice: RESULT_CLOSE_OTHER_BUFFERS_HANDLERS,
	|ops| ops.close_other_buffers()
);

// Alternative binding for close_buffer
#[distributed_slice(KEYBINDINGS_WINDOW)]
static KB_CLOSE_BUFFER_ALT: KeyBindingDef = KeyBindingDef {
	mode: evildoer_manifest::keybindings::BindingMode::Window,
	key: Key::char('c'),
	action: "close_buffer",
	priority: 100,
};
