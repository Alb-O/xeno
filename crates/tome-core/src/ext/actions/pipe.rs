//! Pipe and shell command actions.

use linkme::distributed_slice;

use super::{ACTIONS, ActionContext, ActionDef, ActionMode, ActionResult};

macro_rules! action {
	($name:ident, $id:expr, $desc:expr, $handler:expr) => {
		#[distributed_slice(ACTIONS)]
		static $name: ActionDef = ActionDef {
			name: $id,
			description: $desc,
			handler: $handler,
		};
	};
}

action!(
	ACTION_PIPE_REPLACE,
	"pipe_replace",
	"Pipe selection through command, replace with output",
	|_ctx: &ActionContext| ActionResult::ModeChange(ActionMode::PipeReplace)
);

action!(
	ACTION_PIPE_IGNORE,
	"pipe_ignore",
	"Pipe selection through command, ignore output",
	|_ctx: &ActionContext| ActionResult::ModeChange(ActionMode::PipeIgnore)
);

action!(
	ACTION_INSERT_OUTPUT,
	"insert_output",
	"Insert command output before selection",
	|_ctx: &ActionContext| ActionResult::ModeChange(ActionMode::InsertOutput)
);

action!(
	ACTION_APPEND_OUTPUT,
	"append_output",
	"Append command output after selection",
	|_ctx: &ActionContext| ActionResult::ModeChange(ActionMode::AppendOutput)
);
