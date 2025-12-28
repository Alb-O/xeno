//! Stub handlers for unimplemented features.
//!
//! These handlers exist to provide user feedback when keybindings are pressed
//! for features that haven't been implemented yet. Remove handlers from this
//! file as they get real implementations.

use tome_manifest::editor_ctx::HandleOutcome;
use tome_manifest::result_handler;

use crate::NotifyWARNExt;

result_handler!(RESULT_ALIGN_HANDLERS, HANDLE_ALIGN, "align", |_, ctx, _| {
	ctx.warn("Align not yet implemented");
	HandleOutcome::Handled
});

result_handler!(
	RESULT_COPY_INDENT_HANDLERS,
	HANDLE_COPY_INDENT,
	"copy_indent",
	|_, ctx, _| {
		ctx.warn("Copy indent not yet implemented");
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_TABS_TO_SPACES_HANDLERS,
	HANDLE_TABS_TO_SPACES,
	"tabs_to_spaces",
	|_, ctx, _| {
		ctx.warn("Tabs to spaces not yet implemented");
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_SPACES_TO_TABS_HANDLERS,
	HANDLE_SPACES_TO_TABS,
	"spaces_to_tabs",
	|_, ctx, _| {
		ctx.warn("Spaces to tabs not yet implemented");
		HandleOutcome::Handled
	}
);

result_handler!(
	RESULT_TRIM_SELECTIONS_HANDLERS,
	HANDLE_TRIM_SELECTIONS,
	"trim_selections",
	|_, ctx, _| {
		ctx.warn("Trim selections not yet implemented");
		HandleOutcome::Handled
	}
);
