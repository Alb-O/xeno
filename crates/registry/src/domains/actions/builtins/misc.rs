use crate::actions::{ActionEffects, ActionResult, ViewEffect, action_handler, edit_op};

action_handler!(add_line_below, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::add_line_below())
));
action_handler!(add_line_above, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::add_line_above())
));
action_handler!(use_selection_as_search, |_ctx| ActionResult::Effects(
	ViewEffect::UseSelectionAsSearch.into()
));

action_handler!(open_palette, |_ctx| ActionResult::Effects(
	crate::actions::UiEffect::OpenPalette.into()
));
