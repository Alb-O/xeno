use xeno_primitives::Mode;

use crate::actions::{ActionEffects, ActionResult, AppEffect, action_handler};

action_handler!(enter_insert, |_ctx| ActionResult::Effects(AppEffect::SetMode(Mode::Insert).into()));
action_handler!(enter_normal, |_ctx| ActionResult::Effects(AppEffect::SetMode(Mode::Normal).into()));
action_handler!(normal_mode, |_ctx| ActionResult::Effects(ActionEffects::mode(Mode::Normal)));
