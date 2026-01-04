//! Gutter rendering using the registry system.

use std::path::Path;

use ropey::RopeSlice;
use xeno_registry::gutter::{
	GutterAnnotations, GutterCell, GutterLineContext, GutterStyle, GutterWidthContext,
	column_widths, total_width,
};
use xeno_registry::themes::Theme;
use xeno_tui::style::{Color, Style};
use xeno_tui::text::Span;

/// Pre-computed gutter column widths for a buffer.
pub struct GutterLayout {
	/// Total width including trailing separator.
	pub total_width: u16,
	/// Individual column widths with their definitions.
	columns: Vec<(u16, &'static xeno_registry::gutter::GutterDef)>,
}

impl GutterLayout {
	/// Creates a new gutter layout for a buffer.
	pub fn new(total_lines: usize, viewport_width: u16) -> Self {
		let ctx = GutterWidthContext {
			total_lines,
			viewport_width,
		};
		Self {
			total_width: total_width(&ctx),
			columns: column_widths(&ctx),
		}
	}

	/// Renders gutter spans for a single line.
	pub fn render_line(
		&self,
		line_idx: usize,
		total_lines: usize,
		cursor_line: usize,
		is_continuation: bool,
		line_text: RopeSlice<'_>,
		path: Option<&Path>,
		annotations: &GutterAnnotations,
		theme: &Theme,
		cursorline_bg: Color,
	) -> Vec<Span<'static>> {
		let is_cursor_line = line_idx == cursor_line;

		let ctx = GutterLineContext {
			line_idx,
			total_lines,
			cursor_line,
			is_cursor_line,
			is_continuation,
			line_text,
			path,
			annotations,
		};

		let mut spans = Vec::with_capacity(self.columns.len() + 1);

		for (width, gutter_def) in &self.columns {
			let cell = (gutter_def.render)(&ctx);
			let span = self.format_cell(cell, *width, is_cursor_line, theme, cursorline_bg);
			spans.push(span);
		}

		// Trailing separator space
		if !self.columns.is_empty() {
			let sep_style = if is_cursor_line {
				Style::default().bg(cursorline_bg)
			} else {
				Style::default()
			};
			spans.push(Span::styled(" ", sep_style));
		}

		spans
	}

	/// Renders gutter spans for empty lines past EOF (the ~ indicator).
	pub fn render_empty_line(&self, theme: &Theme) -> Vec<Span<'static>> {
		if self.columns.is_empty() {
			return Vec::new();
		}

		let dim_color = theme.colors.ui.gutter_fg.blend(theme.colors.ui.bg, 0.5);
		let style = Style::default().fg(dim_color);

		// Right-align ~ within total_width - 1 (for trailing space)
		let width = self.total_width.saturating_sub(1) as usize;
		let text = format!("{:>width$} ", "~", width = width);

		vec![Span::styled(text, style)]
	}

	/// Formats a gutter cell into a styled span.
	fn format_cell(
		&self,
		cell: Option<GutterCell>,
		width: u16,
		is_cursor_line: bool,
		theme: &Theme,
		cursorline_bg: Color,
	) -> Span<'static> {
		let width = width as usize;

		match cell {
			Some(cell) => {
				let fg = match cell.style {
					GutterStyle::Normal | GutterStyle::Cursor => theme.colors.ui.gutter_fg,
					GutterStyle::Dim => theme.colors.ui.gutter_fg.blend(theme.colors.ui.bg, 0.5),
				};

				let mut style = Style::default().fg(fg);
				if is_cursor_line {
					style = style.bg(cursorline_bg);
				}

				// Right-align the text within the column width
				let text = format!("{:>width$}", cell.text, width = width);
				Span::styled(text, style)
			}
			None => {
				// Empty cell - just spaces
				let style = if is_cursor_line {
					Style::default().bg(cursorline_bg)
				} else {
					Style::default()
				};
				Span::styled(" ".repeat(width), style)
			}
		}
	}
}
