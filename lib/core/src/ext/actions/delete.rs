//! Delete actions for insert mode.

use crate::action;
use crate::ext::actions::ActionResult;

action!(delete_word_back, "Delete word before cursor", |ctx| {
	// This would delete backward to word start
	// For now, we signal this via a special result that the editor handles
	ActionResult::Ok
});

action!(
	delete_word_forward,
	"Delete word after cursor",
	ActionResult::Ok
);

action!(
	delete_to_line_end,
	"Delete from cursor to end of line",
	ActionResult::Ok
);

action!(
	delete_to_line_start,
	"Delete from cursor to start of line",
	ActionResult::Ok
);
