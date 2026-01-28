use xeno_primitives::Mode;

use crate::actions::{ActionEffects, ActionResult, AppEffect, action};

action!(enter_insert, {
	description: "Enter insert mode",
	bindings: r#"normal "i""#,
}, |_ctx| ActionResult::Effects(AppEffect::SetMode(Mode::Insert).into()));

action!(enter_normal, {
	description: "Enter normal mode",
	bindings: r#"insert "esc""#,
}, |_ctx| ActionResult::Effects(AppEffect::SetMode(Mode::Normal).into()));

action!(normal_mode, {
	description: "Switch to normal mode",
	bindings: r#"insert "esc""#,
}, |_ctx| ActionResult::Effects(ActionEffects::mode(Mode::Normal)));

pub(super) const DEFS: &[&crate::actions::ActionDef] = &[
	&ACTION_enter_insert,
	&ACTION_enter_normal,
	&ACTION_normal_mode,
];
