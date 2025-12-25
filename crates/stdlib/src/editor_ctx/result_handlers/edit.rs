//! Edit action result handler.

use tome_manifest::actions::ActionResult;
use tome_manifest::editor_ctx::HandleOutcome;
use tome_manifest::result_handler;

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
