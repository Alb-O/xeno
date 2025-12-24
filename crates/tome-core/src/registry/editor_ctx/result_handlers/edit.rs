//! Edit action result handler.

use crate::registry::actions::ActionResult;
use crate::registry::editor_ctx::HandleOutcome;
use crate::result_handler;

result_handler!(
	RESULT_EDIT_HANDLERS,
	HANDLE_EDIT,
	"edit",
	|r, ctx, extend| {
		if let ActionResult::Edit(action) = r
			&& let Some(edit) = ctx.edit()
		{
			let quit = edit.execute_edit(action, extend);
			return if quit {
				HandleOutcome::Quit
			} else {
				HandleOutcome::Handled
			};
		}
		HandleOutcome::NotHandled
	}
);
