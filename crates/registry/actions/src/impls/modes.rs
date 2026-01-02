use crate::{action, ActionMode, ActionResult};

action!(normal_mode, { description: "Switch to normal mode", bindings: r#"insert "esc""# },
	|_ctx| ActionResult::ModeChange(ActionMode::Normal));
