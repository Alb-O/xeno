//! Mode change result handler.

use tome_manifest::actions::{ActionMode, ActionResult};
use tome_manifest::editor_ctx::HandleOutcome;
use tome_manifest::{Mode, result_handler};

result_handler!(
	RESULT_MODE_CHANGE_HANDLERS,
	HANDLE_MODE_CHANGE,
	"mode_change",
	|r, ctx, _| {
		if let ActionResult::ModeChange(mode) = r {
			let new_mode = match mode {
				ActionMode::Normal => Mode::Normal,
				ActionMode::Insert => Mode::Insert,
				ActionMode::Goto => Mode::Goto,
				ActionMode::View => Mode::View,
			};
			ctx.set_mode(new_mode);
		}
		HandleOutcome::Handled
	}
);
