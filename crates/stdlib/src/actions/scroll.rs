//! Scroll/view actions.

use tome_base::key::{Key, SpecialKey};
use tome_manifest::actions::{ActionResult, EditAction, ScrollAmount, ScrollDir, VisualDirection};
use tome_manifest::bound_action;

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

bound_action!(
	scroll_half_page_up,
	mode: Normal,
	key: Key::ctrl('u'),
	description: "Scroll half page up",
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
	mode: Normal,
	key: Key::ctrl('d'),
	description: "Scroll half page down",
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
	mode: Normal,
	key: Key::special(SpecialKey::PageUp),
	alt_keys: [Key::ctrl('b')],
	description: "Scroll page up",
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
	mode: Normal,
	key: Key::special(SpecialKey::PageDown),
	alt_keys: [Key::ctrl('f')],
	description: "Scroll page down",
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
	mode: Normal,
	key: Key::char('k'),
	alt_keys: [Key::special(SpecialKey::Up)],
	description: "Move up (visual lines)",
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
	mode: Normal,
	key: Key::char('j'),
	alt_keys: [Key::special(SpecialKey::Down)],
	description: "Move down (visual lines)",
	|ctx| {
		ActionResult::Edit(EditAction::MoveVisual {
			direction: VisualDirection::Down,
			count: ctx.count,
			extend: ctx.extend,
		})
	}
);
