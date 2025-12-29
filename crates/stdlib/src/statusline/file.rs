//! File path and modified indicator segment.

use evildoer_manifest::statusline::{RenderedSegment, SegmentPosition, SegmentStyle};
use evildoer_manifest::statusline_segment;

statusline_segment!(SEG_FILE, "file", SegmentPosition::Center, 0, true, |ctx| {
	let path = ctx.path.unwrap_or("[scratch]");
	let modified = if ctx.modified { " [+]" } else { "" };
	let buffer_indicator = if ctx.buffer_count > 1 {
		format!(" [{}/{}]", ctx.buffer_index, ctx.buffer_count)
	} else {
		String::new()
	};
	Some(RenderedSegment {
		text: format!(" {}{}{} ", path, modified, buffer_indicator),
		style: SegmentStyle::Inverted,
	})
});
