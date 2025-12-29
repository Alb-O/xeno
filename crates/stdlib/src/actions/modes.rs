//! Mode-changing actions.

use evildoer_manifest::action;
use evildoer_manifest::actions::{ActionMode, ActionResult};

action!(goto_mode, { description: "Enter goto mode", bindings: r#"normal "g""# },
	|_ctx| ActionResult::ModeChange(ActionMode::Goto));

action!(view_mode, { description: "Enter view mode", bindings: r#"normal "v""# },
	|_ctx| ActionResult::ModeChange(ActionMode::View));

action!(window_mode, { description: "Enter window mode", bindings: r#"normal "ctrl-w""# },
	|_ctx| ActionResult::ModeChange(ActionMode::Window));
