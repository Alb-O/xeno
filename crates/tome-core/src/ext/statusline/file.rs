//! File path and modified indicator segment.

use linkme::distributed_slice;

use crate::ext::statusline::{
    RenderedSegment, STATUSLINE_SEGMENTS, SegmentPosition, SegmentStyle, StatuslineSegmentDef,
};

#[distributed_slice(STATUSLINE_SEGMENTS)]
static SEG_FILE: StatuslineSegmentDef = StatuslineSegmentDef {
    name: "file",
    position: SegmentPosition::Center,
    priority: 0,
    default_enabled: true,
    render: |ctx| {
        let path = ctx.path.unwrap_or("[scratch]");
        let modified = if ctx.modified { " [+]" } else { "" };
        Some(RenderedSegment {
            text: format!(" {}{} ", path, modified),
            style: SegmentStyle::Inverted,
        })
    },
};
