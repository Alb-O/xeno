//! Insert mode entry actions.

use crate::action;
use tome_manifest::actions::{ActionContext, ActionMode, ActionResult};
use tome_manifest::find_motion;

action!(
	insert_before,
	{ description: "Insert before cursor" },
	result: ActionResult::ModeChange(ActionMode::Insert)
);

action!(insert_after, { description: "Insert after cursor" }, handler: insert_after);

fn insert_after(ctx: &ActionContext) -> ActionResult {
	let motion = match find_motion("move_right") {
		Some(m) => m,
		None => return ActionResult::ModeChange(ActionMode::Insert),
	};

	let mut new_selection = ctx.selection.clone();
	new_selection.transform_mut(|range| {
		*range = (motion.handler)(ctx.text, *range, 1, false);
	});

	ActionResult::InsertWithMotion(new_selection)
}

action!(insert_line_start, { description: "Insert at line start (first non-blank)" }, handler: insert_line_start);

fn insert_line_start(ctx: &ActionContext) -> ActionResult {
	let motion = match find_motion("move_first_nonblank") {
		Some(m) => m,
		None => return ActionResult::ModeChange(ActionMode::Insert),
	};

	let mut new_selection = ctx.selection.clone();
	new_selection.transform_mut(|range| {
		*range = (motion.handler)(ctx.text, *range, 1, false);
	});

	ActionResult::InsertWithMotion(new_selection)
}

action!(insert_line_end, { description: "Insert at line end" }, handler: insert_line_end);

fn insert_line_end(ctx: &ActionContext) -> ActionResult {
	let motion = match find_motion("move_line_end") {
		Some(m) => m,
		None => return ActionResult::ModeChange(ActionMode::Insert),
	};

	let mut new_selection = ctx.selection.clone();
	new_selection.transform_mut(|range| {
		*range = (motion.handler)(ctx.text, *range, 1, false);
	});

	ActionResult::InsertWithMotion(new_selection)
}
