//! Cursor position segment.

use crate::statusline::{RenderedSegment, SegmentStyle, segment};

segment!(position, {
	position: Right,
	description: "Cursor line and column position",
}, |ctx| {
	Some(RenderedSegment {
		text: format!(" {}:{} ", ctx.line, ctx.col),
		style: SegmentStyle::Inverted,
	})
});
