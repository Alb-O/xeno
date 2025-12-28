//! Insert mode entry actions.

use tome_base::key::Key;
use tome_manifest::actions::{ActionContext, ActionMode, ActionResult};
use tome_manifest::{bound_action, find_motion};

bound_action!(
	insert_before,
	description: "Insert before cursor",
	bindings: [Normal => [Key::char('i')]],
	|_ctx| ActionResult::ModeChange(ActionMode::Insert)
);

bound_action!(
	insert_after,
	description: "Insert after cursor",
	bindings: [Normal => [Key::char('a')]],
	handler: insert_after_impl
);

fn insert_after_impl(ctx: &ActionContext) -> ActionResult {
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

bound_action!(
	insert_line_start,
	description: "Insert at line start (first non-blank)",
	bindings: [Normal => [Key::char('I')]],
	handler: insert_line_start_impl
);

fn insert_line_start_impl(ctx: &ActionContext) -> ActionResult {
	let motion = match find_motion("first_nonwhitespace") {
		Some(m) => m,
		None => return ActionResult::ModeChange(ActionMode::Insert),
	};

	let mut new_selection = ctx.selection.clone();
	new_selection.transform_mut(|range| {
		*range = (motion.handler)(ctx.text, *range, 1, false);
	});

	ActionResult::InsertWithMotion(new_selection)
}

bound_action!(
	insert_line_end,
	description: "Insert at line end",
	bindings: [Normal => [Key::char('A')]],
	handler: insert_line_end_impl
);

fn insert_line_end_impl(ctx: &ActionContext) -> ActionResult {
	let motion = match find_motion("line_end") {
		Some(m) => m,
		None => return ActionResult::ModeChange(ActionMode::Insert),
	};

	let mut new_selection = ctx.selection.clone();
	new_selection.transform_mut(|range| {
		*range = (motion.handler)(ctx.text, *range, 1, false);
	});

	ActionResult::InsertWithMotion(new_selection)
}
