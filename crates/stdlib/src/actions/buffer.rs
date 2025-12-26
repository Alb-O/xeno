//! Buffer and split management actions.

use super::ActionResult;
use crate::action;

action!(
	split_horizontal,
	{ description: "Split window horizontally" },
	result: ActionResult::SplitHorizontal
);

action!(
	split_vertical,
	{ description: "Split window vertically" },
	result: ActionResult::SplitVertical
);

action!(
	buffer_next,
	{ description: "Switch to next buffer" },
	result: ActionResult::BufferNext
);

action!(
	buffer_prev,
	{ description: "Switch to previous buffer" },
	result: ActionResult::BufferPrev
);

action!(
	close_buffer,
	{ description: "Close current buffer" },
	result: ActionResult::CloseBuffer
);

action!(
	close_other_buffers,
	{ description: "Close all other buffers" },
	result: ActionResult::CloseOtherBuffers
);

action!(
	focus_left,
	{ description: "Focus split to the left" },
	result: ActionResult::FocusLeft
);

action!(
	focus_right,
	{ description: "Focus split to the right" },
	result: ActionResult::FocusRight
);

action!(
	focus_up,
	{ description: "Focus split above" },
	result: ActionResult::FocusUp
);

action!(
	focus_down,
	{ description: "Focus split below" },
	result: ActionResult::FocusDown
);

action!(
	split_terminal_horizontal,
	{ description: "Open terminal in horizontal split" },
	result: ActionResult::SplitTerminalHorizontal
);

action!(
	split_terminal_vertical,
	{ description: "Open terminal in vertical split" },
	result: ActionResult::SplitTerminalVertical
);
