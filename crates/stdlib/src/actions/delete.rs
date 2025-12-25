//! Delete actions for insert mode.

use crate::action;
use tome_manifest::actions::ActionResult;

action!(delete_word_back, { description: "Delete word before cursor" }, |ctx| {
	// This would delete backward to word start
	// For now, we signal this via a special result that the editor handles
	ActionResult::Ok
});

action!(
	delete_word_forward,
	{ description: "Delete word after cursor" },
	result: ActionResult::Ok
);

action!(
	delete_to_line_end,
	{ description: "Delete from cursor to end of line" },
	result: ActionResult::Ok
);

action!(
	delete_to_line_start,
	{ description: "Delete from cursor to start of line" },
	result: ActionResult::Ok
);
