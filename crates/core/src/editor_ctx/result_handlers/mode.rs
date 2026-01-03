//! Mode change result handler.

use xeno_registry::{ActionMode, ActionResult, HandleOutcome, Mode, result_handler};

result_handler!(
	RESULT_MODE_CHANGE_HANDLERS,
	HANDLE_MODE_CHANGE,
	"mode_change",
	|r, ctx, _| {
		if let ActionResult::ModeChange(mode) = r {
			let new_mode = match mode {
				ActionMode::Normal => Mode::Normal,
				ActionMode::Insert => Mode::Insert,
			};
			ctx.set_mode(new_mode);
		}
		HandleOutcome::Handled
	}
);
