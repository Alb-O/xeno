//! File type indicator segment.

use evildoer_manifest::statusline::{RenderedSegment, SegmentPosition, SegmentStyle};
use evildoer_manifest::statusline_segment;

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
