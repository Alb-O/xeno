use crate::actions::{ActionEffects, ActionResult, ViewEffect, action, edit_op};

action!(add_line_below, { description: "Add empty line below cursor" },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::add_line_below())));

action!(add_line_above, { description: "Add empty line above cursor" },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::add_line_above())));

action!(use_selection_as_search, { description: "Use current selection as search pattern" },
	|_ctx| ActionResult::Effects(ViewEffect::UseSelectionAsSearch.into()));

action!(open_palette, {
	description: "Open command palette",
	bindings: r#"normal ":""#,
}, |_ctx| ActionResult::Effects(crate::actions::UiEffect::OpenPalette.into()));

pub(super) const DEFS: &[&crate::actions::ActionDef] = &[
	&ACTION_add_line_below,
	&ACTION_add_line_above,
	&ACTION_use_selection_as_search,
	&ACTION_open_palette,
];
