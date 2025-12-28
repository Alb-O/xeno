//! Edit action result handler.

use evildoer_manifest::actions::ActionResult;
use evildoer_manifest::editor_ctx::HandleOutcome;
use evildoer_manifest::result_handler;

result_handler!(
	RESULT_EDIT_HANDLERS,
	HANDLE_EDIT,
	"edit",
	|r, ctx, extend| {
		if let ActionResult::Edit(action) = r
			&& let Some(edit) = ctx.edit()
		{
			edit.execute_edit(action, extend);
			return HandleOutcome::Handled;
		}
		HandleOutcome::NotHandled
	}
);
