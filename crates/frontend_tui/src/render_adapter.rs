use xeno_editor::RenderLine;
use xeno_tui::text::{Line, Span};

/// Converts backend-neutral render lines into TUI line primitives.
pub fn to_tui_lines(lines: Vec<RenderLine<'static>>) -> Vec<Line<'static>> {
	lines
		.into_iter()
		.map(|line| {
			let spans: Vec<Span<'static>> = line.spans.into_iter().map(|span| Span::styled(span.content, span.style)).collect();
			let mut tui_line = Line::from(spans);
			if let Some(style) = line.style {
				tui_line = tui_line.style(style);
			}
			tui_line
		})
		.collect()
}
