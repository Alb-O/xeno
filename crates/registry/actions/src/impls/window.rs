//! Split and window management actions.
//!
//! Split names follow Vim/Helix conventions based on the divider line orientation:
//! - `split_horizontal`: horizontal divider → windows stacked top/bottom
//! - `split_vertical`: vertical divider → windows side-by-side left/right
//!
//! Bindings use hierarchical key sequences under `ctrl-w`:
//! - `s h/v` - Split horizontal/vertical
//! - `f h/j/k/l` - Focus directions
//! - `f n/p` - Buffer next/previous
//! - `c c/o` - Close current/others

use xeno_primitives::direction::{Axis, SeqDirection, SpatialDirection};

use crate::{ActionResult, Effect, action};

action!(split_horizontal, {
	description: "Split horizontal",
	short_desc: "Horizontal",
	bindings: r#"normal "ctrl-w s h""#,
}, |_ctx| ActionResult::Effects(Effect::Split(Axis::Horizontal).into()));

action!(split_vertical, {
	description: "Split vertical",
	short_desc: "Vertical",
	bindings: r#"normal "ctrl-w s v""#,
}, |_ctx| ActionResult::Effects(Effect::Split(Axis::Vertical).into()));

action!(focus_left, {
	description: "Focus left",
	short_desc: "Left",
	bindings: r#"normal "ctrl-w f h""#,
}, |_ctx| ActionResult::Effects(Effect::FocusSplit(SpatialDirection::Left).into()));

action!(focus_down, {
	description: "Focus down",
	short_desc: "Down",
	bindings: r#"normal "ctrl-w f j""#,
}, |_ctx| ActionResult::Effects(Effect::FocusSplit(SpatialDirection::Down).into()));

action!(focus_up, {
	description: "Focus up",
	short_desc: "Up",
	bindings: r#"normal "ctrl-w f k""#,
}, |_ctx| ActionResult::Effects(Effect::FocusSplit(SpatialDirection::Up).into()));

action!(focus_right, {
	description: "Focus right",
	short_desc: "Right",
	bindings: r#"normal "ctrl-w f l""#,
}, |_ctx| ActionResult::Effects(Effect::FocusSplit(SpatialDirection::Right).into()));

action!(buffer_next, {
	description: "Next buffer",
	short_desc: "Next",
	bindings: r#"normal "ctrl-w f n""#,
}, |_ctx| ActionResult::Effects(Effect::FocusBuffer(SeqDirection::Next).into()));

action!(buffer_prev, {
	description: "Previous buffer",
	short_desc: "Previous",
	bindings: r#"normal "ctrl-w f p""#,
}, |_ctx| ActionResult::Effects(Effect::FocusBuffer(SeqDirection::Prev).into()));

action!(close_split, {
	description: "Close current split",
	short_desc: "Current",
	bindings: r#"normal "ctrl-w c c""#,
}, |_ctx| ActionResult::Effects(Effect::CloseSplit.into()));

action!(close_other_buffers, {
	description: "Close other buffers",
	short_desc: "Others",
	bindings: r#"normal "ctrl-w c o""#,
}, |_ctx| ActionResult::Effects(Effect::CloseOtherBuffers.into()));
