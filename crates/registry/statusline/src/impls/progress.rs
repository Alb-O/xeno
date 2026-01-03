//! Percentage/progress segment.

use crate::{RenderedSegment, SegmentPosition, SegmentStyle, statusline_segment};

statusline_segment!(
	SEG_PROGRESS,
	"progress",
	SegmentPosition::Right,
	20,
	true,
	|ctx| {
		let percent = if ctx.total_lines == 0 {
			100
		} else {
			(ctx.line * 100) / ctx.total_lines
		};

		let text = if ctx.line == 1 {
			" Top ".to_string()
		} else if ctx.line >= ctx.total_lines {
			" Bot ".to_string()
		} else {
			format!(" {}% ", percent)
		};

		Some(RenderedSegment {
			text,
			style: SegmentStyle::Dim,
		})
	}
);
