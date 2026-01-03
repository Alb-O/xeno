//! Read-only indicator segment.

use crate::{RenderedSegment, SegmentPosition, SegmentStyle, statusline_segment};

statusline_segment!(
	SEG_READONLY,
	"readonly",
	SegmentPosition::Left,
	5,
	true,
	|ctx| {
		if ctx.readonly {
			Some(RenderedSegment {
				text: " READ-ONLY ".to_string(),
				style: SegmentStyle::Warning,
			})
		} else {
			None
		}
	}
);
