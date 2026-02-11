use xeno_primitives::direction::{Axis, SeqDirection, SpatialDirection};

use crate::actions::{ActionResult, AppEffect, action_handler};

action_handler!(split_horizontal, |_ctx| ActionResult::Effects(AppEffect::Split(Axis::Horizontal).into()));
action_handler!(split_vertical, |_ctx| ActionResult::Effects(AppEffect::Split(Axis::Vertical).into()));
action_handler!(focus_left, |_ctx| ActionResult::Effects(AppEffect::FocusSplit(SpatialDirection::Left).into()));
action_handler!(focus_down, |_ctx| ActionResult::Effects(AppEffect::FocusSplit(SpatialDirection::Down).into()));
action_handler!(focus_up, |_ctx| ActionResult::Effects(AppEffect::FocusSplit(SpatialDirection::Up).into()));
action_handler!(focus_right, |_ctx| ActionResult::Effects(AppEffect::FocusSplit(SpatialDirection::Right).into()));
action_handler!(buffer_next, |_ctx| ActionResult::Effects(AppEffect::FocusBuffer(SeqDirection::Next).into()));
action_handler!(buffer_prev, |_ctx| ActionResult::Effects(AppEffect::FocusBuffer(SeqDirection::Prev).into()));
action_handler!(close_split, |_ctx| ActionResult::Effects(AppEffect::CloseSplit.into()));
action_handler!(close_other_buffers, |_ctx| ActionResult::Effects(AppEffect::CloseOtherBuffers.into()));
