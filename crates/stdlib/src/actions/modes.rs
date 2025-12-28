//! Mode-changing actions.

use tome_base::key::Key;
use tome_manifest::actions::{ActionMode, ActionResult};
use tome_manifest::bound_action;

bound_action!(
	goto_mode,
	mode: Normal,
	key: Key::char('g'),
	description: "Enter goto mode",
	|_ctx| ActionResult::ModeChange(ActionMode::Goto)
);

bound_action!(
	view_mode,
	mode: Normal,
	key: Key::char('v'),
	description: "Enter view mode",
	|_ctx| ActionResult::ModeChange(ActionMode::View)
);

bound_action!(
	window_mode,
	mode: Normal,
	key: Key::ctrl('w'),
	description: "Enter window mode",
	|_ctx| ActionResult::ModeChange(ActionMode::Window)
);
