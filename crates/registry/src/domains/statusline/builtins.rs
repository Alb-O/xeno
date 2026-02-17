//! Built-in statusline segment implementations.

use crate::segment_handler;
use crate::statusline::{RenderedSegment, SegmentStyle};

segment_handler!(mode, |ctx| {
	Some(RenderedSegment {
		text: format!(" {} ", ctx.mode_name.to_uppercase()),
		style: SegmentStyle::Mode,
	})
});

segment_handler!(count, |ctx| {
	if ctx.count > 0 {
		Some(RenderedSegment {
			text: format!(" {} ", ctx.count),
			style: SegmentStyle::Inverted,
		})
	} else {
		None
	}
});

segment_handler!(file, |ctx| {
	let path = ctx.path.unwrap_or("[No Name]");
	let file = xeno_file_display::FileItem::new(std::path::Path::new(path));
	let presentation = xeno_file_display::present_file(file, xeno_file_display::FileDisplayContext::default());
	let modified = if ctx.modified { " [+]" } else { "" };
	Some(RenderedSegment {
		text: format!(" {} {}{} ", presentation.icon(), path, modified),
		style: SegmentStyle::Normal,
	})
});

segment_handler!(readonly, |ctx| {
	if ctx.readonly {
		Some(RenderedSegment {
			text: " [RO] ".to_string(),
			style: SegmentStyle::Warning,
		})
	} else {
		None
	}
});

segment_handler!(filetype, |ctx| {
	ctx.file_type.map(|ft| RenderedSegment {
		text: format!(" {} ", ft),
		style: SegmentStyle::Dim,
	})
});

segment_handler!(position, |ctx| {
	Some(RenderedSegment {
		text: format!(" {}:{} ", ctx.line, ctx.col),
		style: SegmentStyle::Normal,
	})
});

segment_handler!(progress, |ctx| {
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
	crate::statusline::register_compiled(builder);
}

fn register_builtins_reg(builder: &mut crate::db::builder::RegistryDbBuilder) -> Result<(), crate::db::builder::RegistryError> {
	register_builtins(builder);
	Ok(())
}

inventory::submit!(crate::db::builtins::BuiltinsReg {
	ordinal: 80,
	f: register_builtins_reg,
});
