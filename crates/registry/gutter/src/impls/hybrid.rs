//! Hybrid line numbers: absolute on cursor line, relative elsewhere.

use crate::{GutterCell, gutter};

/// Computes dynamic width based on total line count.
fn line_number_width(ctx: &crate::GutterWidthContext) -> u16 {
	(ctx.total_lines.max(1).ilog10() as u16 + 1).max(3)
}

gutter!(hybrid_line_numbers, {
	description: "Absolute on cursor line, relative elsewhere",
	priority: 0,
	width: Dynamic(line_number_width),
	enabled: false
}, |ctx| {
	if ctx.is_continuation {
		Some(GutterCell::new("┆", None, true))
	} else {
		let n = if ctx.is_cursor_line { ctx.line_idx + 1 } else { ctx.line_idx.abs_diff(ctx.cursor_line) };
		Some(GutterCell::new(format!("{n}"), None, false))
	}
});
