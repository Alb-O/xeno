//! Diff line numbers gutter column.
//!
//! Displays source file line numbers for diff files in `old│new` format
//! based on hunk context.

use crate::{GutterCell, GutterStyle, gutter};

gutter!(diff_line_numbers, {
	description: "Line numbers from diff hunks",
	priority: 0,
	width: Dynamic(|_| 7),
	enabled: false
}, |ctx| {
	let style = if ctx.is_cursor_line { GutterStyle::Cursor } else { GutterStyle::Normal };

	match (ctx.annotations.diff_old_line, ctx.annotations.diff_new_line) {
		(Some(o), Some(n)) => Some(GutterCell { text: format!("{o:>3}│{n:<3}"), style }),
		(Some(o), None) => Some(GutterCell { text: format!("{o:>3}│   "), style }),
		(None, Some(n)) => Some(GutterCell { text: format!("   │{n:<3}"), style }),
		(None, None) => Some(GutterCell { text: "   │   ".into(), style: GutterStyle::Dim }),
	}
});
