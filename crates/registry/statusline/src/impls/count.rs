//! Count prefix segment.

use crate::{statusline_segment, RenderedSegment, SegmentPosition, SegmentStyle};

statusline_segment!(SEG_COUNT, "count", SegmentPosition::Left, 10, true, |ctx| {
	if ctx.count > 0 {
		Some(RenderedSegment {
			text: format!(" {} ", ctx.count),
			style: SegmentStyle::Normal,
		})
	} else {
		None
	}
});
