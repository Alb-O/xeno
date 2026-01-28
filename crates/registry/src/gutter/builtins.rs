//! Built-in gutter column implementations.

use crate::gutter::{GutterCell, GutterWidthContext, gutter};

/// Computes dynamic width based on total line count for absolute line numbers.
fn line_number_width(ctx: &GutterWidthContext) -> u16 {
	(ctx.total_lines.max(1).ilog10() as u16 + 1).max(3)
}

gutter!(line_numbers, {
	description: "Absolute line numbers",
	priority: 0,
	width: Dynamic(line_number_width)
}, |ctx| {
	if ctx.is_continuation {
		Some(GutterCell::new("┆", None, true))
	} else {
		Some(GutterCell::new(format!("{}", ctx.line_idx + 1), None, false))
	}
});

gutter!(relative, {
	description: "Relative line numbers",
	priority: 0,
	width: Dynamic(line_number_width),
	enabled: false
}, |ctx| {
	if ctx.is_continuation {
		Some(GutterCell::new("┆", None, true))
	} else {
		let rel = (ctx.line_idx as isize - ctx.cursor_line as isize).abs() as usize;
		Some(GutterCell::new(format!("{}", rel), None, false))
	}
});

gutter!(hybrid, {
	description: "Hybrid line numbers",
	priority: 0,
	width: Dynamic(line_number_width),
	enabled: false
}, |ctx| {
	if ctx.is_continuation {
		Some(GutterCell::new("┆", None, true))
	} else if ctx.is_cursor_line {
		Some(GutterCell::new(format!("{}", ctx.line_idx + 1), None, false))
	} else {
		let rel = (ctx.line_idx as isize - ctx.cursor_line as isize).abs() as usize;
		Some(GutterCell::new(format!("{}", rel), None, false))
	}
});

gutter!(diff_line_numbers, {
	description: "Diff line numbers",
	priority: 0,
	width: Fixed(4),
	enabled: false
}, |ctx| {
	if ctx.is_continuation {
		Some(GutterCell::new("┆", None, true))
	} else {
		let line = if let Some(n) = ctx.annotations.diff_new_line {
			format!("{:<3}", n)
		} else if let Some(n) = ctx.annotations.diff_old_line {
			format!("{:<3}", n)
		} else {
			"   ".to_string()
		};
		Some(GutterCell::new(line, None, false))
	}
});

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

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_gutter(&GUTTER_LINE_NUMBERS);
	builder.register_gutter(&GUTTER_RELATIVE);
	builder.register_gutter(&GUTTER_HYBRID);
	builder.register_gutter(&GUTTER_DIFF_LINE_NUMBERS);
	builder.register_gutter(&GUTTER_SIGNS);
}
