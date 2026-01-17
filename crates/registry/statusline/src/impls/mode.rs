//! Mode indicator segment.

use crate::{RenderedSegment, SegmentStyle, segment};

segment!(mode, {
	position: Left,
	description: "Current editor mode",
}, |ctx| {
	Some(RenderedSegment {
		text: format!(" {} ", ctx.mode_name),
		style: SegmentStyle::Mode,
	})
});
