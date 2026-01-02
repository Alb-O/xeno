//! Mode indicator segment.

use crate::{statusline_segment, RenderedSegment, SegmentPosition, SegmentStyle};

statusline_segment!(SEG_MODE, "mode", SegmentPosition::Left, 0, true, |ctx| {
	Some(RenderedSegment {
		text: format!(" {} ", ctx.mode_name),
		style: SegmentStyle::Mode,
	})
});
