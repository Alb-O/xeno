use xeno_base::Mode;

use crate::{ActionEffects, ActionResult, action};

action!(normal_mode, {
	description: "Switch to normal mode",
	bindings: r#"insert "esc""#,
}, |_ctx| ActionResult::Effects(ActionEffects::mode(Mode::Normal)));
