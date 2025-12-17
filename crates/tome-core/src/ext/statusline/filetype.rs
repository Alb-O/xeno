//! File type indicator segment.

use linkme::distributed_slice;

use crate::ext::statusline::{
    RenderedSegment, STATUSLINE_SEGMENTS, SegmentPosition, SegmentStyle, StatuslineSegmentDef,
};

#[distributed_slice(STATUSLINE_SEGMENTS)]
static SEG_FILETYPE: StatuslineSegmentDef = StatuslineSegmentDef {
    name: "filetype",
    position: SegmentPosition::Right,
    priority: 10,
    default_enabled: true,
    render: |ctx| {
        ctx.file_type.map(|ft| RenderedSegment {
            text: format!(" {} ", ft),
            style: SegmentStyle::Dim,
        })
    },
};
