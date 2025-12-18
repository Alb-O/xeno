//! File path and modified indicator segment.

use crate::ext::statusline::{RenderedSegment, SegmentPosition, SegmentStyle};
use crate::statusline_segment;

statusline_segment!(SEG_FILE, "file", SegmentPosition::Center, 0, true, |ctx| {
	let path = ctx.path.unwrap_or("[scratch]");
	let modified = if ctx.modified { " [+]" } else { "" };
	Some(RenderedSegment {
		text: format!(" {}{} ", path, modified),
		style: SegmentStyle::Inverted,
	})
});
