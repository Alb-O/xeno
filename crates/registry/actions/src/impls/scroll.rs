//! Scroll/view actions.

use crate::{
	ActionEffects, ActionResult, EditAction, ScrollAmount, ScrollDir, VisualDirection, action,
};

action!(scroll_up, {
	description: "View scroll up",
	short_desc: "Scroll up",
	bindings: r#"normal "z k""#,
}, |ctx| {
	ActionResult::Effects(ActionEffects::edit(EditAction::Scroll {
		direction: ScrollDir::Up,
		amount: ScrollAmount::Line(ctx.count),
		extend: ctx.extend,
	}))
});

action!(scroll_down, {
	description: "View scroll down",
	short_desc: "Scroll down",
	bindings: r#"normal "z j""#,
}, |ctx| {
	ActionResult::Effects(ActionEffects::edit(EditAction::Scroll {
		direction: ScrollDir::Down,
		amount: ScrollAmount::Line(ctx.count),
		extend: ctx.extend,
	}))
});

action!(scroll_half_page_up, {
	description: "Scroll half page up",
	bindings: r#"normal "ctrl-u""#,
}, |ctx| ActionResult::Effects(ActionEffects::edit(EditAction::Scroll {
	direction: ScrollDir::Up,
	amount: ScrollAmount::HalfPage,
	extend: ctx.extend,
})));

action!(scroll_half_page_down, {
	description: "Scroll half page down",
	bindings: r#"normal "ctrl-d""#,
}, |ctx| ActionResult::Effects(ActionEffects::edit(EditAction::Scroll {
	direction: ScrollDir::Down,
	amount: ScrollAmount::HalfPage,
	extend: ctx.extend,
})));

action!(scroll_page_up, {
	description: "Scroll page up",
	bindings: r#"normal "pageup" "ctrl-b"
insert "pageup""#,
}, |ctx| ActionResult::Effects(ActionEffects::edit(EditAction::Scroll {
	direction: ScrollDir::Up,
	amount: ScrollAmount::FullPage,
	extend: ctx.extend,
})));

action!(scroll_page_down, {
	description: "Scroll page down",
	bindings: r#"normal "pagedown" "ctrl-f"
insert "pagedown""#,
}, |ctx| ActionResult::Effects(ActionEffects::edit(EditAction::Scroll {
	direction: ScrollDir::Down,
	amount: ScrollAmount::FullPage,
	extend: ctx.extend,
})));

action!(move_up_visual, {
	description: "Move up (visual lines)",
	bindings: r#"normal "k" "up"
insert "up""#,
}, |ctx| ActionResult::Effects(ActionEffects::edit(EditAction::MoveVisual {
	direction: VisualDirection::Up,
	count: ctx.count,
	extend: ctx.extend,
})));

action!(move_down_visual, {
	description: "Move down (visual lines)",
	bindings: r#"normal "j" "down"
insert "down""#,
}, |ctx| ActionResult::Effects(ActionEffects::edit(EditAction::MoveVisual {
	direction: VisualDirection::Down,
	count: ctx.count,
	extend: ctx.extend,
})));
