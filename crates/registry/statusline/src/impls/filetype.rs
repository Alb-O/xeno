//! File type indicator segment.

use crate::{RenderedSegment, SegmentPosition, SegmentStyle, statusline_segment};

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
