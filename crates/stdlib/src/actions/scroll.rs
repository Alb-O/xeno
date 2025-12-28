//! Scroll/view actions.

use evildoer_base::key::{Key, SpecialKey};
use evildoer_manifest::actions::{ActionResult, EditAction, ScrollAmount, ScrollDir, VisualDirection};
use evildoer_manifest::{bind, bound_action};

use crate::action;

action!(scroll_up, { description: "Scroll view up" }, |ctx| {
	ActionResult::Edit(EditAction::Scroll {
		direction: ScrollDir::Up,
		amount: ScrollAmount::Line(ctx.count),
		extend: ctx.extend,
	})
});

action!(scroll_down, { description: "Scroll view down" }, |ctx| {
	ActionResult::Edit(EditAction::Scroll {
		direction: ScrollDir::Down,
		amount: ScrollAmount::Line(ctx.count),
		extend: ctx.extend,
	})
});

bind!(scroll_up, View => [Key::char('k')]);
bind!(scroll_down, View => [Key::char('j')]);

bound_action!(
	scroll_half_page_up,
	description: "Scroll half page up",
	bindings: [Normal => [Key::ctrl('u')]],
	|ctx| {
		ActionResult::Edit(EditAction::Scroll {
			direction: ScrollDir::Up,
			amount: ScrollAmount::HalfPage,
			extend: ctx.extend,
		})
	}
);

bound_action!(
	scroll_half_page_down,
	description: "Scroll half page down",
	bindings: [Normal => [Key::ctrl('d')]],
	|ctx| {
		ActionResult::Edit(EditAction::Scroll {
			direction: ScrollDir::Down,
			amount: ScrollAmount::HalfPage,
			extend: ctx.extend,
		})
	}
);

bound_action!(
	scroll_page_up,
	description: "Scroll page up",
	bindings: [
		Normal => [Key::special(SpecialKey::PageUp), Key::ctrl('b')],
		Insert => [Key::special(SpecialKey::PageUp)],
	],
	|ctx| {
		ActionResult::Edit(EditAction::Scroll {
			direction: ScrollDir::Up,
			amount: ScrollAmount::FullPage,
			extend: ctx.extend,
		})
	}
);

bound_action!(
	scroll_page_down,
	description: "Scroll page down",
	bindings: [
		Normal => [Key::special(SpecialKey::PageDown), Key::ctrl('f')],
		Insert => [Key::special(SpecialKey::PageDown)],
	],
	|ctx| {
		ActionResult::Edit(EditAction::Scroll {
			direction: ScrollDir::Down,
			amount: ScrollAmount::FullPage,
			extend: ctx.extend,
		})
	}
);

bound_action!(
	move_up_visual,
	description: "Move up (visual lines)",
	bindings: [
		Normal => [Key::char('k'), Key::special(SpecialKey::Up)],
		Insert => [Key::special(SpecialKey::Up)],
	],
	|ctx| {
		ActionResult::Edit(EditAction::MoveVisual {
			direction: VisualDirection::Up,
			count: ctx.count,
			extend: ctx.extend,
		})
	}
);

bound_action!(
	move_down_visual,
	description: "Move down (visual lines)",
	bindings: [
		Normal => [Key::char('j'), Key::special(SpecialKey::Down)],
		Insert => [Key::special(SpecialKey::Down)],
	],
	|ctx| {
		ActionResult::Edit(EditAction::MoveVisual {
			direction: VisualDirection::Down,
			count: ctx.count,
			extend: ctx.extend,
		})
	}
);
