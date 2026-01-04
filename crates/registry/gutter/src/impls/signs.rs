//! Sign column for diagnostics, breakpoints, and custom markers.

use crate::{GutterCell, GutterStyle, gutter};

gutter!(signs, {
	description: "Sign column for diagnostics and markers",
	priority: -10,
	width: Fixed(2),
	enabled: false
}, |ctx| {
	// Check for custom sign first
	if let Some(sign) = ctx.annotations.sign {
		return Some(GutterCell {
			text: sign.to_string(),
			style: GutterStyle::Normal,
		});
	}

	// Then check diagnostic severity
	match ctx.annotations.diagnostic_severity {
		4 => Some(GutterCell {
			text: "E".into(),
			style: GutterStyle::Normal,
		}),
		3 => Some(GutterCell {
			text: "W".into(),
			style: GutterStyle::Normal,
		}),
		2 => Some(GutterCell {
			text: "I".into(),
			style: GutterStyle::Dim,
		}),
		1 => Some(GutterCell {
			text: "H".into(),
			style: GutterStyle::Dim,
		}),
		_ => None,
	}
});
