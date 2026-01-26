//! Command palette actions.

use crate::actions::{ActionResult, UiEffect, action};

action!(open_palette, {
	description: "Open command palette",
	bindings: r#"normal ":""#,
}, |_ctx| ActionResult::Effects(UiEffect::OpenPalette.into()));
