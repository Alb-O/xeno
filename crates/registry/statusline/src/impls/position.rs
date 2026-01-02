//! Cursor position segment.

use crate::{statusline_segment, RenderedSegment, SegmentPosition, SegmentStyle};

statusline_segment!(
	SEG_POSITION,
	"position",
	SegmentPosition::Right,
	0,
	true,
	|ctx| {
		Some(RenderedSegment {
			text: format!(" {}:{} ", ctx.line, ctx.col),
			style: SegmentStyle::Inverted,
		})
	}
);
