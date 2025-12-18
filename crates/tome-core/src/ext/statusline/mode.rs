//! Mode indicator segment.

use crate::ext::statusline::{RenderedSegment, SegmentPosition, SegmentStyle};
use crate::statusline_segment;

statusline_segment!(SEG_MODE, "mode", SegmentPosition::Left, 0, true, |ctx| {
	Some(RenderedSegment {
		text: format!(" {} ", ctx.mode_name),
		style: SegmentStyle::Mode,
	})
});
