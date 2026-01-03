use crate::{action, ActionMode, ActionResult};

action!(normal_mode, { description: "Switch to normal mode", bindings: r#"insert "esc"
window "esc""# },
	|_ctx| ActionResult::ModeChange(ActionMode::Normal));

action!(window_mode, { description: "Enter window mode", bindings: r#"normal "ctrl-w""# },
	|_ctx| ActionResult::ModeChange(ActionMode::Window));
