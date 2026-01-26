//! Read-only indicator segment.

use crate::statusline::{RenderedSegment, SegmentStyle, segment};

segment!(readonly, {
	position: Left,
	description: "Read-only buffer indicator",
	priority: 5,
}, |ctx| {
	if ctx.readonly {
		Some(RenderedSegment {
			text: " READ-ONLY ".to_string(),
			style: SegmentStyle::Warning,
		})
	} else {
		None
	}
});
