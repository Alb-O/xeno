//! Pipe and shell command actions.

use super::{ActionMode, ActionResult};
use crate::action;

action!(
	pipe_replace,
	{ description: "Pipe selection through command, replace with output" },
	result: ActionResult::ModeChange(ActionMode::PipeReplace)
);

action!(
	pipe_ignore,
	{ description: "Pipe selection through command, ignore output" },
	result: ActionResult::ModeChange(ActionMode::PipeIgnore)
);

action!(
	insert_output,
	{ description: "Insert command output before selection" },
	result: ActionResult::ModeChange(ActionMode::InsertOutput)
);

action!(
	append_output,
	{ description: "Append command output after selection" },
	result: ActionResult::ModeChange(ActionMode::AppendOutput)
);
