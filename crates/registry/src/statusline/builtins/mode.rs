//! Mode indicator segment.

use crate::statusline::{RenderedSegment, SegmentStyle, segment};

segment!(mode, {
	position: Left,
	description: "Current editor mode",
	priority: 30,
}, |ctx| {
	Some(RenderedSegment {
		text: format!(" {} ", ctx.mode_name),
		style: SegmentStyle::Mode,
	})
});
