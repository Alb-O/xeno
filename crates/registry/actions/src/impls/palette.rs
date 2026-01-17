//! Command palette actions.

use crate::{ActionResult, UiEffect, action};

action!(open_palette, {
	description: "Open command palette",
	bindings: r#"normal ":""#,
}, |_ctx| ActionResult::Effects(UiEffect::OpenPalette.into()));
