//! Command palette actions.

use crate::{ActionResult, Effect, action};

action!(open_palette, {
	description: "Open command palette",
	bindings: r#"normal ":""#,
}, |_ctx| ActionResult::Effects(Effect::OpenPalette.into()));
