//! Scroll/view actions.

use linkme::distributed_slice;

use crate::ext::actions::{
	ACTIONS, ActionDef, ActionResult, EditAction, ScrollAmount, ScrollDir, VisualDirection,
};

#[distributed_slice(ACTIONS)]
static ACTION_SCROLL_UP: ActionDef = ActionDef {
	name: "scroll_up",
	description: "Scroll view up",
	handler: |ctx| {
		ActionResult::Edit(EditAction::Scroll {
			direction: ScrollDir::Up,
			amount: ScrollAmount::Line(ctx.count),
			extend: ctx.extend,
		})
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_SCROLL_DOWN: ActionDef = ActionDef {
	name: "scroll_down",
	description: "Scroll view down",
	handler: |ctx| {
		ActionResult::Edit(EditAction::Scroll {
			direction: ScrollDir::Down,
			amount: ScrollAmount::Line(ctx.count),
			extend: ctx.extend,
		})
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_SCROLL_HALF_PAGE_UP: ActionDef = ActionDef {
	name: "scroll_half_page_up",
	description: "Scroll half page up",
	handler: |ctx| {
		ActionResult::Edit(EditAction::Scroll {
			direction: ScrollDir::Up,
			amount: ScrollAmount::HalfPage,
			extend: ctx.extend,
		})
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_SCROLL_HALF_PAGE_DOWN: ActionDef = ActionDef {
	name: "scroll_half_page_down",
	description: "Scroll half page down",
	handler: |ctx| {
		ActionResult::Edit(EditAction::Scroll {
			direction: ScrollDir::Down,
			amount: ScrollAmount::HalfPage,
			extend: ctx.extend,
		})
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_SCROLL_PAGE_UP: ActionDef = ActionDef {
	name: "scroll_page_up",
	description: "Scroll page up",
	handler: |ctx| {
		ActionResult::Edit(EditAction::Scroll {
			direction: ScrollDir::Up,
			amount: ScrollAmount::FullPage,
			extend: ctx.extend,
		})
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_SCROLL_PAGE_DOWN: ActionDef = ActionDef {
	name: "scroll_page_down",
	description: "Scroll page down",
	handler: |ctx| {
		ActionResult::Edit(EditAction::Scroll {
			direction: ScrollDir::Down,
			amount: ScrollAmount::FullPage,
			extend: ctx.extend,
		})
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_CENTER_CURSOR: ActionDef = ActionDef {
	name: "center_cursor",
	description: "Center cursor in view",
	handler: |_ctx| ActionResult::Ok, // TODO: Needs viewport info
};

#[distributed_slice(ACTIONS)]
static ACTION_CURSOR_TO_TOP: ActionDef = ActionDef {
	name: "cursor_to_top",
	description: "Move view so cursor is at top",
	handler: |_ctx| ActionResult::Ok, // TODO: Needs viewport info
};

#[distributed_slice(ACTIONS)]
static ACTION_CURSOR_TO_BOTTOM: ActionDef = ActionDef {
	name: "cursor_to_bottom",
	description: "Move view so cursor is at bottom",
	handler: |_ctx| ActionResult::Ok, // TODO: Needs viewport info
};

#[distributed_slice(ACTIONS)]
static ACTION_MOVE_UP_VISUAL: ActionDef = ActionDef {
	name: "move_up_visual",
	description: "Move up (visual lines)",
	handler: |ctx| {
		ActionResult::Edit(EditAction::MoveVisual {
			direction: VisualDirection::Up,
			count: ctx.count,
			extend: ctx.extend,
		})
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_MOVE_DOWN_VISUAL: ActionDef = ActionDef {
	name: "move_down_visual",
	description: "Move down (visual lines)",
	handler: |ctx| {
		ActionResult::Edit(EditAction::MoveVisual {
			direction: VisualDirection::Down,
			count: ctx.count,
			extend: ctx.extend,
		})
	},
};
