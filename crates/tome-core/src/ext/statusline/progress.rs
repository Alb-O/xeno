//! Percentage/progress segment.

use linkme::distributed_slice;

use crate::ext::statusline::{
    RenderedSegment, STATUSLINE_SEGMENTS, SegmentPosition, SegmentStyle, StatuslineSegmentDef,
};

#[distributed_slice(STATUSLINE_SEGMENTS)]
static SEG_PROGRESS: StatuslineSegmentDef = StatuslineSegmentDef {
    name: "progress",
    position: SegmentPosition::Right,
    priority: 20,
    default_enabled: true,
    render: |ctx| {
        let percent = if ctx.total_lines == 0 {
            100
        } else {
            (ctx.line * 100) / ctx.total_lines
        };

        let text = if ctx.line == 1 {
            " Top ".to_string()
        } else if ctx.line >= ctx.total_lines {
            " Bot ".to_string()
        } else {
            format!(" {}% ", percent)
        };

        Some(RenderedSegment {
            text,
            style: SegmentStyle::Dim,
        })
    },
};
