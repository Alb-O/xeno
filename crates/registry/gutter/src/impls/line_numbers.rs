//! Absolute line numbers gutter column.

use crate::{GutterCell, GutterStyle, gutter};

/// Computes dynamic width based on total line count.
///
/// Formula: `log10(lines) + 1` with minimum of 3 characters.
/// The trailing separator space is added by the gutter layout.
fn line_number_width(ctx: &crate::GutterWidthContext) -> u16 {
	(ctx.total_lines.max(1).ilog10() as u16 + 1).max(3)
}

gutter!(line_numbers, {
	description: "Absolute line numbers",
	priority: 0,
	width: Dynamic(line_number_width)
}, |ctx| {
	if ctx.is_continuation {
		Some(GutterCell {
			text: "\u{2506}".into(), // ┆ box drawings light triple dash vertical
			style: GutterStyle::Dim,
		})
	} else {
		Some(GutterCell {
			text: format!("{}", ctx.line_idx + 1),
			style: if ctx.is_cursor_line {
				GutterStyle::Cursor
			} else {
				GutterStyle::Normal
			},
		})
	}
});
