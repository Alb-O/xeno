//! Insert mode entry actions.

use linkme::distributed_slice;

use crate::ext::actions::{ACTIONS, ActionContext, ActionDef, ActionMode, ActionResult};
use crate::ext::find_motion;

fn insert_before(_ctx: &ActionContext) -> ActionResult {
	ActionResult::ModeChange(ActionMode::Insert)
}

#[distributed_slice(ACTIONS)]
static ACTION_INSERT_BEFORE: ActionDef = ActionDef {
	name: "insert_before",
	description: "Insert before cursor",
	handler: insert_before,
};

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

#[distributed_slice(ACTIONS)]
static ACTION_INSERT_AFTER: ActionDef = ActionDef {
	name: "insert_after",
	description: "Insert after cursor",
	handler: insert_after,
};

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

#[distributed_slice(ACTIONS)]
static ACTION_INSERT_LINE_START: ActionDef = ActionDef {
	name: "insert_line_start",
	description: "Insert at line start (first non-blank)",
	handler: insert_line_start,
};

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

#[distributed_slice(ACTIONS)]
static ACTION_INSERT_LINE_END: ActionDef = ActionDef {
	name: "insert_line_end",
	description: "Insert at line end",
	handler: insert_line_end,
};
