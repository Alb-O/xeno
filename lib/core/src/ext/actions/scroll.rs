//! Scroll/view actions.

use crate::action;
use crate::ext::actions::{ActionResult, EditAction, ScrollAmount, ScrollDir, VisualDirection};

action!(scroll_up, "Scroll view up", |ctx| {
	ActionResult::Edit(EditAction::Scroll {
		direction: ScrollDir::Up,
		amount: ScrollAmount::Line(ctx.count),
		extend: ctx.extend,
	})
});

action!(scroll_down, "Scroll view down", |ctx| {
	ActionResult::Edit(EditAction::Scroll {
		direction: ScrollDir::Down,
		amount: ScrollAmount::Line(ctx.count),
		extend: ctx.extend,
	})
});

action!(scroll_half_page_up, "Scroll half page up", |ctx| {
	ActionResult::Edit(EditAction::Scroll {
		direction: ScrollDir::Up,
		amount: ScrollAmount::HalfPage,
		extend: ctx.extend,
	})
});

action!(scroll_half_page_down, "Scroll half page down", |ctx| {
	ActionResult::Edit(EditAction::Scroll {
		direction: ScrollDir::Down,
		amount: ScrollAmount::HalfPage,
		extend: ctx.extend,
	})
});

action!(scroll_page_up, "Scroll page up", |ctx| {
	ActionResult::Edit(EditAction::Scroll {
		direction: ScrollDir::Up,
		amount: ScrollAmount::FullPage,
		extend: ctx.extend,
	})
});

action!(scroll_page_down, "Scroll page down", |ctx| {
	ActionResult::Edit(EditAction::Scroll {
		direction: ScrollDir::Down,
		amount: ScrollAmount::FullPage,
		extend: ctx.extend,
	})
});

action!(center_cursor, "Center cursor in view", ActionResult::Ok); // TODO: Needs viewport info
action!(
	cursor_to_top,
	"Move view so cursor is at top",
	ActionResult::Ok
); // TODO: Needs viewport info
action!(
	cursor_to_bottom,
	"Move view so cursor is at bottom",
	ActionResult::Ok
); // TODO: Needs viewport info

action!(move_up_visual, "Move up (visual lines)", |ctx| {
	ActionResult::Edit(EditAction::MoveVisual {
		direction: VisualDirection::Up,
		count: ctx.count,
		extend: ctx.extend,
	})
});

action!(move_down_visual, "Move down (visual lines)", |ctx| {
	ActionResult::Edit(EditAction::MoveVisual {
		direction: VisualDirection::Down,
		count: ctx.count,
		extend: ctx.extend,
	})
});
