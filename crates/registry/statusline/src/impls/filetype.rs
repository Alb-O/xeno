//! File type indicator segment.

use crate::{RenderedSegment, SegmentStyle, segment};

segment!(filetype, {
	position: Right,
	description: "Detected file type",
	priority: 10,
}, |ctx| {
	ctx.file_type.map(|ft| RenderedSegment {
		text: format!(" {} ", ft),
		style: SegmentStyle::Dim,
	})
});
