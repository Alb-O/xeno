//! Mode indicator segment.

use linkme::distributed_slice;

use crate::ext::statusline::{
    RenderedSegment, SegmentPosition, SegmentStyle, StatuslineSegmentDef, STATUSLINE_SEGMENTS,
};

#[distributed_slice(STATUSLINE_SEGMENTS)]
static SEG_MODE: StatuslineSegmentDef = StatuslineSegmentDef {
    name: "mode",
    position: SegmentPosition::Left,
    priority: 0,
    default_enabled: true,
    render: |ctx| {
        Some(RenderedSegment {
            text: format!(" {} ", ctx.mode_name),
            style: SegmentStyle::Mode,
        })
    },
};
