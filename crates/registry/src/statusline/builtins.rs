//! Built-in statusline segment implementations.

use crate::statusline::{RenderedSegment, SegmentStyle, segment};

segment!(mode, {
	position: Left,
	description: "Mode indicator",
	priority: 100,
}, |ctx| {
	Some(RenderedSegment {
		text: format!(" {} ", ctx.mode_name.to_uppercase()),
		style: SegmentStyle::Mode,
	})
});

segment!(count, {
	position: Left,
	description: "Numeric prefix count",
	priority: 90,
}, |ctx| {
	if ctx.count > 0 {
		Some(RenderedSegment {
			text: format!(" {} ", ctx.count),
			style: SegmentStyle::Inverted,
		})
	} else {
		None
	}
});

segment!(file, {
	position: Left,
	description: "Current filename and modified flag",
	priority: 80,
}, |ctx| {
	let path = ctx.path.unwrap_or("[No Name]");
	let modified = if ctx.modified { " [+]" } else { "" };
	Some(RenderedSegment {
		text: format!(" {}{} ", path, modified),
		style: SegmentStyle::Normal,
	})
});

segment!(readonly, {
	position: Left,
	description: "Read-only indicator",
	priority: 75,
}, |ctx| {
	if ctx.readonly {
		Some(RenderedSegment {
			text: " [RO] ".to_string(),
			style: SegmentStyle::Warning,
		})
	} else {
		None
	}
});

segment!(filetype, {
	position: Right,
	description: "Detected file type",
	priority: 50,
}, |ctx| {
	ctx.file_type.map(|ft| RenderedSegment {
		text: format!(" {} ", ft),
		style: SegmentStyle::Dim,
	})
});

segment!(position, {
	position: Right,
	description: "Cursor position (line:col)",
	priority: 100,
}, |ctx| {
	Some(RenderedSegment {
		text: format!(" {}:{} ", ctx.line, ctx.col),
		style: SegmentStyle::Normal,
	})
});

segment!(progress, {
	position: Right,
	description: "Document progress percentage",
	priority: 90,
}, |ctx| {
	let pct = if ctx.total_lines > 1 {
		(ctx.line - 1) * 100 / (ctx.total_lines - 1)
	} else {
		100
	};
	Some(RenderedSegment {
		text: format!(" {}% ", pct),
		style: SegmentStyle::Dim,
	})
});

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_statusline_segment(&SEG_MODE);
	builder.register_statusline_segment(&SEG_COUNT);
	builder.register_statusline_segment(&SEG_FILE);
	builder.register_statusline_segment(&SEG_READONLY);
	builder.register_statusline_segment(&SEG_FILETYPE);
	builder.register_statusline_segment(&SEG_POSITION);
	builder.register_statusline_segment(&SEG_PROGRESS);
}
