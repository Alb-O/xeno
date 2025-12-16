//! Cursor position segment.

use linkme::distributed_slice;

use crate::ext::statusline::{
    RenderedSegment, SegmentPosition, SegmentStyle, StatuslineSegmentDef, STATUSLINE_SEGMENTS,
};

#[distributed_slice(STATUSLINE_SEGMENTS)]
static SEG_POSITION: StatuslineSegmentDef = StatuslineSegmentDef {
    name: "position",
    position: SegmentPosition::Right,
    priority: 0,
    default_enabled: true,
    render: |ctx| {
        Some(RenderedSegment {
            text: format!(" {}:{} ", ctx.line, ctx.col),
            style: SegmentStyle::Inverted,
        })
    },
};
