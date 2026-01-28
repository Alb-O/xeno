use crate::actions::{ActionEffects, ActionResult, AppEffect, ViewEffect, action};

action!(search, {
	description: "Open search prompt (forward)",
	bindings: r#"normal "/""#,
}, |_ctx| ActionResult::Effects(ActionEffects::from_effect(
	AppEffect::OpenSearchPrompt { reverse: false }.into(),
)));

action!(search_reverse, {
	description: "Open search prompt (reverse)",
	bindings: r#"normal "?""#,
}, |_ctx| ActionResult::Effects(ActionEffects::from_effect(
	AppEffect::OpenSearchPrompt { reverse: true }.into(),
)));

action!(search_next, {
	description: "Repeat last search (same direction)",
	bindings: r#"normal "n""#,
}, |ctx| ActionResult::Effects(ActionEffects::from_effect(
	ViewEffect::SearchRepeat {
		flip: false,
		add_selection: false,
		extend: ctx.extend,
	}.into()
)));

action!(search_prev, {
	description: "Repeat last search (opposite direction)",
	bindings: r#"normal "N""#,
}, |ctx| ActionResult::Effects(ActionEffects::from_effect(
	ViewEffect::SearchRepeat {
		flip: true,
		add_selection: false,
		extend: ctx.extend,
	}.into()
)));

pub(super) const DEFS: &[&crate::actions::ActionDef] = &[
	&ACTION_search,
	&ACTION_search_reverse,
	&ACTION_search_next,
	&ACTION_search_prev,
];
