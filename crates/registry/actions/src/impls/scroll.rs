//! Scroll/view actions.

use crate::{action, ActionResult, EditAction, ScrollAmount, ScrollDir, VisualDirection};

action!(scroll_up, { description: "Scroll view up", bindings: r#"normal "z k""# }, |ctx| {
	ActionResult::Edit(EditAction::Scroll {
		direction: ScrollDir::Up,
		amount: ScrollAmount::Line(ctx.count),
		extend: ctx.extend,
	})
});

action!(scroll_down, { description: "Scroll view down", bindings: r#"normal "z j""# }, |ctx| {
	ActionResult::Edit(EditAction::Scroll {
		direction: ScrollDir::Down,
		amount: ScrollAmount::Line(ctx.count),
		extend: ctx.extend,
	})
});

action!(scroll_half_page_up, {
	description: "Scroll half page up",
	bindings: r#"normal "ctrl-u""#,
}, |ctx| ActionResult::Edit(EditAction::Scroll {
	direction: ScrollDir::Up,
	amount: ScrollAmount::HalfPage,
	extend: ctx.extend,
}));

action!(scroll_half_page_down, {
	description: "Scroll half page down",
	bindings: r#"normal "ctrl-d""#,
}, |ctx| ActionResult::Edit(EditAction::Scroll {
	direction: ScrollDir::Down,
	amount: ScrollAmount::HalfPage,
	extend: ctx.extend,
}));

action!(scroll_page_up, {
	description: "Scroll page up",
	bindings: r#"normal "pageup" "ctrl-b"
insert "pageup""#,
}, |ctx| ActionResult::Edit(EditAction::Scroll {
	direction: ScrollDir::Up,
	amount: ScrollAmount::FullPage,
	extend: ctx.extend,
}));

action!(scroll_page_down, {
	description: "Scroll page down",
	bindings: r#"normal "pagedown" "ctrl-f"
insert "pagedown""#,
}, |ctx| ActionResult::Edit(EditAction::Scroll {
	direction: ScrollDir::Down,
	amount: ScrollAmount::FullPage,
	extend: ctx.extend,
}));

action!(move_up_visual, {
	description: "Move up (visual lines)",
	bindings: r#"normal "k" "up"
insert "up""#,
}, |ctx| ActionResult::Edit(EditAction::MoveVisual {
	direction: VisualDirection::Up,
	count: ctx.count,
	extend: ctx.extend,
}));

action!(move_down_visual, {
	description: "Move down (visual lines)",
	bindings: r#"normal "j" "down"
insert "down""#,
}, |ctx| ActionResult::Edit(EditAction::MoveVisual {
	direction: VisualDirection::Down,
	count: ctx.count,
	extend: ctx.extend,
}));
