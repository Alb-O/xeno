use xeno_primitives::direction::{Axis, SeqDirection, SpatialDirection};

use crate::actions::{ActionResult, AppEffect, action};

action!(split_horizontal, {
	description: "Split horizontal",
	short_desc: "Horizontal",
	bindings: r#"normal "ctrl-w s h""#,
}, |_ctx| ActionResult::Effects(AppEffect::Split(Axis::Horizontal).into()));

action!(split_vertical, {
	description: "Split vertical",
	short_desc: "Vertical",
	bindings: r#"normal "ctrl-w s v""#,
}, |_ctx| ActionResult::Effects(AppEffect::Split(Axis::Vertical).into()));

action!(focus_left, {
	description: "Focus left",
	short_desc: "Left",
	bindings: r#"normal "ctrl-w f h""#,
}, |_ctx| ActionResult::Effects(AppEffect::FocusSplit(SpatialDirection::Left).into()));

action!(focus_down, {
	description: "Focus down",
	short_desc: "Down",
	bindings: r#"normal "ctrl-w f j""#,
}, |_ctx| ActionResult::Effects(AppEffect::FocusSplit(SpatialDirection::Down).into()));

action!(focus_up, {
	description: "Focus up",
	short_desc: "Up",
	bindings: r#"normal "ctrl-w f k""#,
}, |_ctx| ActionResult::Effects(AppEffect::FocusSplit(SpatialDirection::Up).into()));

action!(focus_right, {
	description: "Focus right",
	short_desc: "Right",
	bindings: r#"normal "ctrl-w f l""#,
}, |_ctx| ActionResult::Effects(AppEffect::FocusSplit(SpatialDirection::Right).into()));

action!(buffer_next, {
	description: "Next buffer",
	short_desc: "Next",
	bindings: r#"normal "ctrl-w f n""#,
}, |_ctx| ActionResult::Effects(AppEffect::FocusBuffer(SeqDirection::Next).into()));

action!(buffer_prev, {
	description: "Previous buffer",
	short_desc: "Previous",
	bindings: r#"normal "ctrl-w f p""#,
}, |_ctx| ActionResult::Effects(AppEffect::FocusBuffer(SeqDirection::Prev).into()));

action!(close_split, {
	description: "Close current split",
	short_desc: "Current",
	bindings: r#"normal "ctrl-w c c""#,
}, |_ctx| ActionResult::Effects(AppEffect::CloseSplit.into()));

action!(close_other_buffers, {
	description: "Close other buffers",
	short_desc: "Others",
	bindings: r#"normal "ctrl-w c o""#,
}, |_ctx| ActionResult::Effects(AppEffect::CloseOtherBuffers.into()));

pub(super) const DEFS: &[&crate::actions::ActionDef] = &[
	&ACTION_split_horizontal,
	&ACTION_split_vertical,
	&ACTION_focus_left,
	&ACTION_focus_down,
	&ACTION_focus_up,
	&ACTION_focus_right,
	&ACTION_buffer_next,
	&ACTION_buffer_prev,
	&ACTION_close_split,
	&ACTION_close_other_buffers,
];
