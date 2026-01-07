//! Search result handlers.

use xeno_registry::{ActionResult, HandleOutcome, result_handler};

result_handler!(
	RESULT_SEARCH_HANDLERS,
	HANDLE_SEARCH,
	"search",
	|r, ctx, extend| {
		let ActionResult::Search {
			direction,
			add_selection,
		} = r
		else {
			return HandleOutcome::NotHandled;
		};

		if let Some(search) = ctx.search() {
			search.search(*direction, *add_selection, extend);
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
