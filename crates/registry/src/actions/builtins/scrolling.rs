use xeno_primitives::range::Direction;

use crate::actions::{ActionEffects, ActionResult, ScrollAmount, action_handler};

action_handler!(scroll_up, |ctx| ActionResult::Effects(
	ActionEffects::scroll(
		Direction::Backward,
		ScrollAmount::Line(ctx.count),
		ctx.extend,
	)
));

action_handler!(scroll_down, |ctx| ActionResult::Effects(
	ActionEffects::scroll(
		Direction::Forward,
		ScrollAmount::Line(ctx.count),
		ctx.extend,
	)
));

action_handler!(scroll_half_page_up, |ctx| ActionResult::Effects(
	ActionEffects::scroll(Direction::Backward, ScrollAmount::HalfPage, ctx.extend,)
));

action_handler!(scroll_half_page_down, |ctx| ActionResult::Effects(
	ActionEffects::scroll(Direction::Forward, ScrollAmount::HalfPage, ctx.extend,)
));

action_handler!(scroll_page_up, |ctx| ActionResult::Effects(
	ActionEffects::scroll(Direction::Backward, ScrollAmount::FullPage, ctx.extend,)
));

action_handler!(scroll_page_down, |ctx| ActionResult::Effects(
	ActionEffects::scroll(Direction::Forward, ScrollAmount::FullPage, ctx.extend,)
));

action_handler!(move_up_visual, |ctx| ActionResult::Effects(
	ActionEffects::visual_move(Direction::Backward, ctx.count, ctx.extend,)
));

action_handler!(move_down_visual, |ctx| ActionResult::Effects(
	ActionEffects::visual_move(Direction::Forward, ctx.count, ctx.extend,)
));
