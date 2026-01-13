//! Gutter rendering using the registry system.

use std::path::Path;

use ropey::RopeSlice;
use xeno_registry::gutter::{
	GutterAnnotations, GutterCell, GutterLineContext, GutterStyle, GutterWidthContext,
	column_width, column_widths, find as find_gutter, total_width,
};
use xeno_registry::themes::Theme;
use xeno_tui::style::{Color, Style};
use xeno_tui::text::Span;

use super::context::CursorlineConfig;
use crate::window::GutterSelector;

enum GutterLayoutKind {
	Columns(Vec<(u16, &'static xeno_registry::gutter::GutterDef)>),
	Prompt {
		prompt: char,
	},
	Custom {
		width: u16,
		render: fn(&GutterLineContext) -> Option<GutterCell>,
	},
	Hidden,
}

/// Pre-computed gutter column widths for a buffer.
pub struct GutterLayout {
	/// Total width including trailing separator.
	pub total_width: u16,
	kind: GutterLayoutKind,
}

impl GutterLayout {
	/// Creates a new gutter layout for a buffer.
	pub fn new(total_lines: usize, viewport_width: u16) -> Self {
		Self::from_registry(total_lines, viewport_width)
	}

	/// Builds a gutter layout from a selector.
	pub fn from_selector(
		selector: GutterSelector,
		total_lines: usize,
		viewport_width: u16,
	) -> Self {
		match selector {
			GutterSelector::Registry => Self::new(total_lines, viewport_width),
			GutterSelector::Named(names) => Self::from_names(names, total_lines, viewport_width),
			GutterSelector::Hidden => Self::hidden(),
			GutterSelector::Prompt(prompt) => Self::prompt(prompt),
			GutterSelector::Custom { width, render } => Self::custom(width, render),
		}
	}

	/// Creates a gutter layout using registered gutter names.
	pub fn from_names(names: &[&str], total_lines: usize, viewport_width: u16) -> Self {
		let ctx = GutterWidthContext {
			total_lines,
			viewport_width,
		};
		let mut columns: Vec<(u16, &'static xeno_registry::gutter::GutterDef)> = names
			.iter()
			.filter_map(|name| find_gutter(name))
			.map(|def| (column_width(def, &ctx), def))
			.collect();
		columns.sort_by_key(|(_, def)| def.priority);
		let total_width = Self::columns_total_width(&columns);
		Self {
			total_width,
			kind: GutterLayoutKind::Columns(columns),
		}
	}

	/// Creates a hidden gutter layout (zero width).
	pub fn hidden() -> Self {
		Self {
			total_width: 0,
			kind: GutterLayoutKind::Hidden,
		}
	}

	/// Creates a prompt gutter layout using a single character.
	pub fn prompt(prompt: char) -> Self {
		let width = 1;
		Self {
			total_width: Self::column_total_width(width),
			kind: GutterLayoutKind::Prompt { prompt },
		}
	}

	/// Creates a custom gutter layout using a render callback.
	pub fn custom(width: u16, render: fn(&GutterLineContext) -> Option<GutterCell>) -> Self {
		if width == 0 {
			return Self::hidden();
		}

		Self {
			total_width: Self::column_total_width(width),
			kind: GutterLayoutKind::Custom { width, render },
		}
	}

	fn from_registry(total_lines: usize, viewport_width: u16) -> Self {
		let ctx = GutterWidthContext {
			total_lines,
			viewport_width,
		};
		let columns = column_widths(&ctx);
		Self {
			total_width: total_width(&ctx),
			kind: GutterLayoutKind::Columns(columns),
		}
	}

	fn column_total_width(width: u16) -> u16 {
		if width > 0 { width + 1 } else { 0 }
	}

	fn columns_total_width(columns: &[(u16, &'static xeno_registry::gutter::GutterDef)]) -> u16 {
		let columns_width: u16 = columns.iter().map(|(width, _)| *width).sum();
		if columns_width > 0 {
			columns_width + 1
		} else {
			0
		}
	}

	/// Renders gutter spans for a single line.
	#[allow(clippy::too_many_arguments)]
	pub fn render_line(
		&self,
		line_idx: usize,
		total_lines: usize,
		cursorline: &CursorlineConfig,
		is_continuation: bool,
		line_text: RopeSlice<'_>,
		path: Option<&Path>,
		annotations: &GutterAnnotations,
		theme: &Theme,
	) -> Vec<Span<'static>> {
		let is_cursor_line = cursorline.should_highlight(line_idx);

		match &self.kind {
			GutterLayoutKind::Hidden => Vec::new(),
			GutterLayoutKind::Prompt { prompt } => {
				if self.total_width == 0 {
					return Vec::new();
				}
				let cell = if is_continuation {
					None
				} else {
					Some(GutterCell {
						text: prompt.to_string(),
						style: GutterStyle::Normal,
					})
				};
				vec![
					self.format_cell(cell, 1, is_cursor_line, theme, cursorline.bg),
					self.separator_span(is_cursor_line, cursorline.bg),
				]
			}
			GutterLayoutKind::Custom { width, render } => {
				if *width == 0 {
					return Vec::new();
				}
				let ctx = GutterLineContext {
					line_idx,
					total_lines,
					cursor_line: cursorline.line,
					is_cursor_line,
					is_continuation,
					line_text,
					path,
					annotations,
				};
				let cell = render(&ctx);
				vec![
					self.format_cell(cell, *width, is_cursor_line, theme, cursorline.bg),
					self.separator_span(is_cursor_line, cursorline.bg),
				]
			}
			GutterLayoutKind::Columns(columns) => {
				if columns.is_empty() {
					return Vec::new();
				}

				let ctx = GutterLineContext {
					line_idx,
					total_lines,
					cursor_line: cursorline.line,
					is_cursor_line,
					is_continuation,
					line_text,
					path,
					annotations,
				};

				let mut spans = Vec::with_capacity(columns.len() + 1);

				for (width, gutter_def) in columns {
					let cell = (gutter_def.render)(&ctx);
					let span = self.format_cell(cell, *width, is_cursor_line, theme, cursorline.bg);
					spans.push(span);
				}

				spans.push(self.separator_span(is_cursor_line, cursorline.bg));
				spans
			}
		}
	}

	/// Renders gutter spans for empty lines past EOF (the ~ indicator).
	pub fn render_empty_line(&self, theme: &Theme) -> Vec<Span<'static>> {
		match &self.kind {
			GutterLayoutKind::Hidden => Vec::new(),
			GutterLayoutKind::Prompt { .. } | GutterLayoutKind::Custom { .. } => {
				if self.total_width == 0 {
					return Vec::new();
				}
				vec![Span::styled(
					" ".repeat(self.total_width as usize),
					Style::default(),
				)]
			}
			GutterLayoutKind::Columns(columns) => {
				if columns.is_empty() {
					return Vec::new();
				}

				let dim_color = theme.colors.ui.gutter_fg.blend(theme.colors.ui.bg, 0.5);
				let style = Style::default().fg(dim_color);

				// Right-align ~ within total_width - 1 (for trailing space)
				let width = self.total_width.saturating_sub(1) as usize;
				let text = format!("{:>width$} ", "~", width = width);

				vec![Span::styled(text, style)]
			}
		}
	}

	fn separator_span(&self, is_cursor_line: bool, cursorline_bg: Color) -> Span<'static> {
		let sep_style = if is_cursor_line {
			Style::default().bg(cursorline_bg)
		} else {
			Style::default()
		};
		Span::styled(" ", sep_style)
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
					GutterStyle::Dim | GutterStyle::Hint => {
						theme.colors.ui.gutter_fg.blend(theme.colors.ui.bg, 0.5)
					}
					GutterStyle::Error => theme.colors.semantic.error,
					GutterStyle::Warning => theme.colors.semantic.warning,
					GutterStyle::Info => theme.colors.semantic.info,
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
