//! Diff line numbers gutter column.
//!
//! Displays source file line numbers for diff files in `old│new` format
//! based on hunk context.

use crate::gutter::{GutterCell, GutterSegment, gutter};

gutter!(diff_line_numbers, {
	description: "Line numbers from diff hunks",
	priority: 0,
	width: Dynamic(|_| 7),
	enabled: false
}, |ctx| {
	let diff = &ctx.theme.colors.syntax;
	match (ctx.annotations.diff_old_line, ctx.annotations.diff_new_line) {
		(Some(o), Some(n)) => Some(GutterCell::styled(vec![
			GutterSegment { text: format!("{o:>3}"), fg: None, dim: false },
			GutterSegment { text: "│".into(), fg: None, dim: true },
			GutterSegment { text: format!("{n:<3}"), fg: None, dim: false },
		])),
		(Some(o), None) => Some(GutterCell::styled(vec![
			GutterSegment { text: format!("{o:>3}"), fg: diff.diff_minus.fg, dim: false },
			GutterSegment { text: "┆   ".into(), fg: diff.diff_minus.fg, dim: true },
		])),
		(None, Some(n)) => Some(GutterCell::styled(vec![
			GutterSegment { text: "   │".into(), fg: diff.diff_plus.fg, dim: true },
			GutterSegment { text: format!("{n:<3}"), fg: diff.diff_plus.fg, dim: false },
		])),
		(None, None) => Some(GutterCell::new("   │   ", None, true)),
	}
});
