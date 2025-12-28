//! Mode-changing actions.

use evildoer_base::key::Key;
use evildoer_manifest::actions::{ActionMode, ActionResult};
use evildoer_manifest::bound_action;

bound_action!(
	goto_mode,
	description: "Enter goto mode",
	bindings: [Normal => [Key::char('g')]],
	|_ctx| ActionResult::ModeChange(ActionMode::Goto)
);

bound_action!(
	view_mode,
	description: "Enter view mode",
	bindings: [Normal => [Key::char('v')]],
	|_ctx| ActionResult::ModeChange(ActionMode::View)
);

bound_action!(
	window_mode,
	description: "Enter window mode",
	bindings: [Normal => [Key::ctrl('w')]],
	|_ctx| ActionResult::ModeChange(ActionMode::Window)
);
