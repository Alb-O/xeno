//! Built-in gutter column implementations.

use crate::gutter::{GutterCell, GutterWidthContext};
use crate::gutter_handler;

/// Computes dynamic width based on total line count for absolute line numbers.
fn line_number_width(ctx: &GutterWidthContext) -> u16 {
	(ctx.total_lines.max(1).ilog10() as u16 + 1).max(3)
}

gutter_handler!(line_numbers, {
	width: Dynamic(line_number_width),
	render: |ctx| {
		if ctx.is_continuation {
			Some(GutterCell::new("┆", None, true))
		} else {
			Some(GutterCell::new(format!("{}", ctx.line_idx + 1), None, false))
		}
	},
});

gutter_handler!(relative, {
	width: Dynamic(line_number_width),
	render: |ctx| {
		if ctx.is_continuation {
			Some(GutterCell::new("┆", None, true))
		} else {
			let rel = (ctx.line_idx as isize - ctx.cursor_line as isize).unsigned_abs();
			Some(GutterCell::new(format!("{}", rel), None, false))
		}
	},
});

gutter_handler!(hybrid, {
	width: Dynamic(line_number_width),
	render: |ctx| {
		if ctx.is_continuation {
			Some(GutterCell::new("┆", None, true))
		} else if ctx.is_cursor_line {
			Some(GutterCell::new(format!("{}", ctx.line_idx + 1), None, false))
		} else {
			let rel = (ctx.line_idx as isize - ctx.cursor_line as isize).unsigned_abs();
			Some(GutterCell::new(format!("{}", rel), None, false))
		}
	},
});

gutter_handler!(diff_line_numbers, {
	width: Fixed(4),
	render: |ctx| {
		if ctx.is_continuation {
			Some(GutterCell::new("┆", None, true))
		} else {
			let line = if let Some(n) = ctx.annotations.diff_new_line {
				format!("{:<3}", n)
			} else if let Some(n) = ctx.annotations.diff_old_line {
				format!("{:<3}", n)
			} else {
				"   ".to_string()
			};
			Some(GutterCell::new(line, None, false))
		}
	},
});

gutter_handler!(signs, {
	width: Fixed(2),
	render: |ctx| {
		if ctx.is_continuation {
			return None;
		}
		if let Some(sign) = ctx.annotations.sign {
			return Some(GutterCell::new(sign.to_string(), None, false));
		}
		let colors = &ctx.theme.colors.semantic;
		match ctx.annotations.diagnostic_severity {
			4 => Some(GutterCell::new("●", Some(colors.error), false)),
			3 => Some(GutterCell::new("●", Some(colors.warning), false)),
			2 => Some(GutterCell::new("●", Some(colors.info), false)),
			1 => Some(GutterCell::new("●", None, true)),
			_ => None,
		}
	},
});

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	let metadata = crate::kdl::loader::load_gutter_metadata();
	let handlers = inventory::iter::<crate::gutter::GutterHandlerReg>
		.into_iter()
		.map(|r| r.0);
	let linked = crate::kdl::link::link_gutters(&metadata, handlers);

	for def in linked {
		builder.register_linked_gutter(def);
	}
}
