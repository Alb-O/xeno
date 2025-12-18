//! Scratch buffer result handlers.

use crate::ext::actions::ActionResult;
use crate::ext::editor_ctx::HandleOutcome;
use crate::result_handler;

result_handler!(
	RESULT_OPEN_SCRATCH_HANDLERS,
	HANDLE_OPEN_SCRATCH,
	"open_scratch",
	|r, ctx, _| {
		if let ActionResult::OpenScratch { focus } = r
			&& let Some(scratch) = ctx.scratch()
		{
			scratch.open(*focus);
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_CLOSE_SCRATCH_HANDLERS,
	HANDLE_CLOSE_SCRATCH,
	"close_scratch",
	|_, ctx, _| {
		if let Some(scratch) = ctx.scratch() {
			scratch.close();
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_TOGGLE_SCRATCH_HANDLERS,
	HANDLE_TOGGLE_SCRATCH,
	"toggle_scratch",
	|_, ctx, _| {
		if let Some(scratch) = ctx.scratch() {
			scratch.toggle();
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_EXECUTE_SCRATCH_HANDLERS,
	HANDLE_EXECUTE_SCRATCH,
	"execute_scratch",
	|_, ctx, _| {
		if let Some(scratch) = ctx.scratch()
			&& scratch.execute()
		{
			return HandleOutcome::Quit;
		}
		HandleOutcome::Handled
	}
);
