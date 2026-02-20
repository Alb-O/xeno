use crate::actions::{ActionEffects, ActionResult, AppEffect, ViewEffect, action_handler};

action_handler!(search, |_ctx| ActionResult::Effects(ActionEffects::from_effect(
	AppEffect::OpenSearchPrompt { reverse: false }.into(),
)));

action_handler!(search_reverse, |_ctx| ActionResult::Effects(ActionEffects::from_effect(
	AppEffect::OpenSearchPrompt { reverse: true }.into(),
)));

action_handler!(search_next, |ctx| ActionResult::Effects(ActionEffects::from_effect(
	ViewEffect::SearchRepeat {
		flip: false,
		add_selection: false,
		extend: ctx.extend,
	}
	.into()
)));

action_handler!(search_prev, |ctx| ActionResult::Effects(ActionEffects::from_effect(
	ViewEffect::SearchRepeat {
		flip: true,
		add_selection: false,
		extend: ctx.extend,
	}
	.into()
)));
