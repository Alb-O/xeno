//! Mode-changing actions.

use evildoer_manifest::actions::{ActionMode, ActionResult};
use evildoer_manifest::bound_action;

bound_action!(goto_mode, description: "Enter goto mode", bindings: r#"normal "g""#,
	|_ctx| ActionResult::ModeChange(ActionMode::Goto));

bound_action!(view_mode, description: "Enter view mode", bindings: r#"normal "v""#,
	|_ctx| ActionResult::ModeChange(ActionMode::View));

bound_action!(window_mode, description: "Enter window mode", bindings: r#"normal "ctrl-w""#,
	|_ctx| ActionResult::ModeChange(ActionMode::Window));
