//! Mode change result handler.

use evildoer_manifest::actions::{ActionMode, ActionResult};
use evildoer_manifest::editor_ctx::HandleOutcome;
use evildoer_manifest::{Mode, result_handler};

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
				ActionMode::Window => Mode::Window,
			};
			ctx.set_mode(new_mode);
		}
		HandleOutcome::Handled
	}
);
