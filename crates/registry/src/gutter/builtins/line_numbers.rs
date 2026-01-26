//! Absolute line numbers gutter column.

use crate::gutter::{GutterCell, gutter};

/// Computes dynamic width based on total line count.
///
/// Formula: `log10(lines) + 1` with minimum of 3 characters.
/// The trailing separator space is added by the gutter layout.
fn line_number_width(ctx: &crate::gutter::GutterWidthContext) -> u16 {
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
