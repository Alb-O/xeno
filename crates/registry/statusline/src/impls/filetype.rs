//! File type indicator segment.

use crate::{statusline_segment, RenderedSegment, SegmentPosition, SegmentStyle};

statusline_segment!(
	SEG_FILETYPE,
	"filetype",
	SegmentPosition::Right,
	10,
	true,
	|ctx| {
		ctx.file_type.map(|ft| RenderedSegment {
			text: format!(" {} ", ft),
			style: SegmentStyle::Dim,
		})
	}
);
