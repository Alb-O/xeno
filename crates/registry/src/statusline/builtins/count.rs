//! Count prefix segment.

use crate::statusline::{RenderedSegment, SegmentStyle, segment};

segment!(count, {
	position: Left,
	description: "Numeric count prefix",
	priority: 20,
}, |ctx| {
	if ctx.count > 0 {
		Some(RenderedSegment {
			text: format!(" {} ", ctx.count),
			style: SegmentStyle::Normal,
		})
	} else {
		None
	}
});
