//! Cursor position segment.

use crate::statusline_segment;
use crate::ext::statusline::{RenderedSegment, SegmentPosition, SegmentStyle};

statusline_segment!(
    SEG_POSITION,
    "position",
    SegmentPosition::Right,
    0,
    true,
    |ctx| {
        Some(RenderedSegment {
            text: format!(" {}:{} ", ctx.line, ctx.col),
            style: SegmentStyle::Inverted,
        })
    }
);
