//! Relative line numbers gutter column.

use crate::{GutterCell, gutter};

/// Computes dynamic width based on total line count.
fn line_number_width(ctx: &crate::GutterWidthContext) -> u16 {
	(ctx.total_lines.max(1).ilog10() as u16 + 1).max(3)
}

gutter!(relative_line_numbers, {
	description: "Relative line numbers from cursor",
	priority: 0,
	width: Dynamic(line_number_width),
	enabled: false
}, |ctx| {
	if ctx.is_continuation {
		Some(GutterCell::new("┆", None, true))
	} else {
		Some(GutterCell::new(format!("{}", ctx.line_idx.abs_diff(ctx.cursor_line)), None, false))
	}
});
