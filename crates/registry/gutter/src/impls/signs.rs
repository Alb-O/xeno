//! Sign column for diagnostics, breakpoints, and custom markers.

use crate::{GutterCell, gutter};

gutter!(signs, {
	description: "Sign column for diagnostics and markers",
	priority: -10,
	width: Fixed(2),
	enabled: true
}, |ctx| {
	if ctx.is_continuation {
		return None;
	}
	if let Some(sign) = ctx.annotations.sign {
		return Some(GutterCell::new(sign.to_string(), None, false));
	}
	let colors = &ctx.theme.colors.semantic;
	match ctx.annotations.diagnostic_severity {
		4 => Some(GutterCell::new("●", Some(colors.error), false)),
		3 => Some(GutterCell::new("●", Some(colors.warning), false)),
		2 => Some(GutterCell::new("●", Some(colors.info), false)),
		1 => Some(GutterCell::new("●", None, true)),
		_ => None,
	}
});
