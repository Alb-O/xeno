//! Motion actions that wrap MotionDefs into ActionDefs.

use linkme::distributed_slice;

use crate::ext::actions::{ActionContext, ActionDef, ActionResult, ACTIONS};
use crate::ext::find_motion;

fn motion_action(ctx: &ActionContext, motion_name: &str) -> ActionResult {
    let motion = match find_motion(motion_name) {
        Some(m) => m,
        None => return ActionResult::Error(format!("Unknown motion: {}", motion_name)),
    };

    let mut new_selection = ctx.selection.clone();
    new_selection.transform_mut(|range| {
        *range = (motion.handler)(ctx.text, *range, ctx.count, ctx.extend);
    });
    ActionResult::Motion(new_selection)
}

macro_rules! motion_action {
    ($static_name:ident, $action_name:expr, $motion_name:expr, $description:expr) => {
        paste::paste! {
            fn [<handler_ $static_name>](ctx: &ActionContext) -> ActionResult {
                motion_action(ctx, $motion_name)
            }

            #[distributed_slice(ACTIONS)]
            static [<ACTION_ $static_name:upper>]: ActionDef = ActionDef {
                name: $action_name,
                description: $description,
                handler: [<handler_ $static_name>],
            };
        }
    };
}

motion_action!(action_move_left, "move_left", "move_left", "Move left");
motion_action!(action_move_right, "move_right", "move_right", "Move right");
motion_action!(action_move_up, "move_up", "move_up", "Move up");
motion_action!(action_move_down, "move_down", "move_down", "Move down");
motion_action!(action_move_line_start, "move_line_start", "line_start", "Move to line start");
motion_action!(action_move_line_end, "move_line_end", "line_end", "Move to line end");
motion_action!(action_move_first_nonblank, "move_first_nonblank", "first_nonwhitespace", "Move to first non-blank");
motion_action!(action_next_word_start, "next_word_start", "next_word_start", "Move to next word start");
motion_action!(action_next_word_end, "next_word_end", "next_word_end", "Move to next word end");
motion_action!(action_prev_word_start, "prev_word_start", "prev_word_start", "Move to previous word start");
motion_action!(action_prev_word_end, "prev_word_end", "prev_word_end", "Move to previous word end");
motion_action!(action_next_long_word_start, "next_long_word_start", "next_long_word_start", "Move to next WORD start");
motion_action!(action_next_long_word_end, "next_long_word_end", "next_long_word_end", "Move to next WORD end");
motion_action!(action_prev_long_word_start, "prev_long_word_start", "prev_long_word_start", "Move to previous WORD start");
motion_action!(action_document_start, "document_start", "document_start", "Move to document start");
motion_action!(action_document_end, "document_end", "document_end", "Move to document end");
