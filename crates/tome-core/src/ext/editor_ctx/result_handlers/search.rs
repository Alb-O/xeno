//! Search result handlers.

use crate::ext::actions::ActionResult;
use crate::ext::editor_ctx::HandleOutcome;
use crate::result_handler;

result_handler!(
	RESULT_SEARCH_NEXT_HANDLERS,
	HANDLE_SEARCH_NEXT,
	"search_next",
	|r, ctx, extend| {
		if let ActionResult::SearchNext { add_selection } = r
			&& let Some(search) = ctx.search()
		{
			search.search_next(*add_selection, extend);
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_SEARCH_PREV_HANDLERS,
	HANDLE_SEARCH_PREV,
	"search_prev",
	|r, ctx, extend| {
		if let ActionResult::SearchPrev { add_selection } = r
			&& let Some(search) = ctx.search()
		{
			search.search_prev(*add_selection, extend);
		}
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_USE_SELECTION_SEARCH_HANDLERS,
	HANDLE_USE_SELECTION_AS_SEARCH,
	"use_selection_as_search",
	|_, ctx, _| {
		if let Some(search) = ctx.search() {
			search.use_selection_as_pattern();
		}
		HandleOutcome::Handled
	}
);
