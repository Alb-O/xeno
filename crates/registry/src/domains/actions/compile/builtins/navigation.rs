use xeno_primitives::{MotionId, motion_ids};

use crate::actions::{ActionEffects, ActionResult, ScreenPosition, action_handler};

pub fn cursor_motion(ctx: &crate::actions::ActionContext, id: MotionId) -> ActionResult {
	ActionResult::Effects(ActionEffects::cursor_motion(id, ctx.count, ctx.extend))
}

pub fn selection_motion(ctx: &crate::actions::ActionContext, id: MotionId) -> ActionResult {
	ActionResult::Effects(ActionEffects::selection_motion(id, ctx.count, ctx.extend))
}

fn word_motion(ctx: &crate::actions::ActionContext, id: MotionId) -> ActionResult {
	ActionResult::Effects(ActionEffects::word_motion(id, ctx.count, ctx.extend))
}

action_handler!(move_left, |ctx| cursor_motion(ctx, motion_ids::LEFT));
action_handler!(move_right, |ctx| cursor_motion(ctx, motion_ids::RIGHT));
action_handler!(move_up, |ctx| cursor_motion(ctx, motion_ids::UP));
action_handler!(move_down, |ctx| cursor_motion(ctx, motion_ids::DOWN));
action_handler!(move_line_start, |ctx| cursor_motion(ctx, motion_ids::LINE_START));
action_handler!(move_line_end, |ctx| cursor_motion(ctx, motion_ids::LINE_END));
action_handler!(next_word_start, |ctx| word_motion(ctx, motion_ids::NEXT_WORD_START));
action_handler!(prev_word_start, |ctx| word_motion(ctx, motion_ids::PREV_WORD_START));
action_handler!(next_word_end, |ctx| word_motion(ctx, motion_ids::NEXT_WORD_END));
action_handler!(next_long_word_start, |ctx| word_motion(ctx, motion_ids::NEXT_LONG_WORD_START));
action_handler!(prev_long_word_start, |ctx| word_motion(ctx, motion_ids::PREV_LONG_WORD_START));
action_handler!(next_long_word_end, |ctx| word_motion(ctx, motion_ids::NEXT_LONG_WORD_END));
action_handler!(select_word_forward, |ctx| selection_motion(ctx, motion_ids::NEXT_WORD_START));
action_handler!(select_word_backward, |ctx| selection_motion(ctx, motion_ids::PREV_WORD_START));
action_handler!(select_word_end, |ctx| selection_motion(ctx, motion_ids::NEXT_WORD_END));
action_handler!(next_paragraph, |ctx| cursor_motion(ctx, motion_ids::NEXT_PARAGRAPH));
action_handler!(prev_paragraph, |ctx| cursor_motion(ctx, motion_ids::PREV_PARAGRAPH));
action_handler!(document_start, |ctx| cursor_motion(ctx, motion_ids::DOCUMENT_START));
action_handler!(document_end, |ctx| cursor_motion(ctx, motion_ids::DOCUMENT_END));
action_handler!(goto_line_start, |ctx| cursor_motion(ctx, motion_ids::LINE_START));
action_handler!(goto_line_end, |ctx| cursor_motion(ctx, motion_ids::LINE_END));
action_handler!(goto_first_nonwhitespace, |ctx| cursor_motion(ctx, motion_ids::FIRST_NONWHITESPACE));

action_handler!(move_top_screen, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Top, ctx.count))
});

action_handler!(move_middle_screen, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Middle, ctx.count))
});

action_handler!(move_bottom_screen, |ctx| {
	ActionResult::Effects(ActionEffects::screen_motion(ScreenPosition::Bottom, ctx.count))
});

action_handler!(goto_next_hunk, |ctx| cursor_motion(ctx, motion_ids::NEXT_HUNK));
action_handler!(goto_prev_hunk, |ctx| cursor_motion(ctx, motion_ids::PREV_HUNK));
