//! File path and modified indicator segment.

use crate::{RenderedSegment, SegmentStyle, segment};

segment!(file, {
	position: Center,
	description: "File path with modified and buffer indicators",
}, |ctx| {
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
