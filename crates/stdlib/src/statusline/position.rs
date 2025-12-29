//! Cursor position segment.

use evildoer_manifest::statusline::{RenderedSegment, SegmentPosition, SegmentStyle};
use evildoer_manifest::statusline_segment;

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
