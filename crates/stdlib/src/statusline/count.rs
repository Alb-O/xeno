//! Count prefix segment.

use tome_manifest::statusline::{RenderedSegment, SegmentPosition, SegmentStyle};
use crate::statusline_segment;

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
