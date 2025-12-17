//! Count prefix segment.

use linkme::distributed_slice;

use crate::ext::statusline::{
    RenderedSegment, STATUSLINE_SEGMENTS, SegmentPosition, SegmentStyle, StatuslineSegmentDef,
};

#[distributed_slice(STATUSLINE_SEGMENTS)]
static SEG_COUNT: StatuslineSegmentDef = StatuslineSegmentDef {
    name: "count",
    position: SegmentPosition::Left,
    priority: 10,
    default_enabled: true,
    render: |ctx| {
        if ctx.count > 0 {
            Some(RenderedSegment {
                text: format!(" {} ", ctx.count),
                style: SegmentStyle::Normal,
            })
        } else {
            None
        }
    },
};
