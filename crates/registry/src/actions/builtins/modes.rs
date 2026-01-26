use xeno_primitives::Mode;

use crate::actions::{ActionEffects, ActionResult, action};

action!(normal_mode, {
	description: "Switch to normal mode",
	bindings: r#"insert "esc""#,
}, |_ctx| ActionResult::Effects(ActionEffects::mode(Mode::Normal)));
