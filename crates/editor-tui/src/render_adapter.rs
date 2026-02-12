use xeno_editor::render_api::RenderLine;
use xeno_tui::text::Line;

/// Converts backend-neutral render lines into TUI line primitives.
pub fn to_tui_lines(lines: Vec<RenderLine<'static>>) -> Vec<Line<'static>> {
	lines.into_iter().map(RenderLine::into_text_line).collect()
}
