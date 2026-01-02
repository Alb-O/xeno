use evildoer_registry::{
	render_position, RenderedSegment, SegmentPosition, SegmentStyle, StatuslineContext,
};
use evildoer_tui::style::{Modifier, Style};
use evildoer_tui::text::{Line, Span};
use evildoer_tui::widgets::{Paragraph, Widget};

use crate::Editor;

impl Editor {
	/// Creates a widget for rendering the status line.
	pub fn render_status_line(&self) -> impl Widget + '_ {
		let buffer_ids = self.buffer_ids();
		let buffer_index = self
			.focused_buffer_id()
			.and_then(|current_id| buffer_ids.iter().position(|&id| id == current_id))
			.unwrap_or(0)
			+ 1;
		let buffer_count = buffer_ids.len();

		// Extract data before creating the context to avoid lifetime issues
		let (path_str, file_type_str, modified, mode_name, line, col, count, total_lines) =
			if let Some(panel) = self.focused_panel_def() {
				(
					None,
					Some(panel.name.to_string()),
					false,
					self.mode_name(),
					0,
					0,
					0,
					0,
				)
			} else {
				let buffer = self.buffer();
				let path_str = buffer
					.path()
					.as_ref()
					.and_then(|p| p.to_str().map(|s| s.to_string()));
				let file_type_str = buffer.file_type();
				let modified = buffer.modified();
				let count = buffer.input.count();
				let total_lines = buffer.doc().content.len_lines();
				let mode_name = self.mode_name();
				let line = self.cursor_line() + 1;
				let col = self.cursor_col() + 1;
				(
					path_str,
					file_type_str,
					modified,
					mode_name,
					line,
					col,
					count,
					total_lines,
				)
			};

		let ctx = StatuslineContext {
			mode_name,
			path: path_str.as_deref(),
			modified,
			line,
			col,
			count,
			total_lines,
			file_type: file_type_str.as_deref(),
			buffer_index,
			buffer_count,
		};

		let mut spans = Vec::new();

		for seg in render_position(SegmentPosition::Left, &ctx) {
			spans.push(self.segment_to_span(&seg));
		}
		for seg in render_position(SegmentPosition::Center, &ctx) {
			spans.push(self.segment_to_span(&seg));
		}
		for seg in render_position(SegmentPosition::Right, &ctx) {
			spans.push(self.segment_to_span(&seg));
		}

		Paragraph::new(Line::from(spans))
	}

	/// Converts a statusline segment to a styled span.
	pub fn segment_to_span(&self, segment: &RenderedSegment) -> Span<'static> {
		let colors = &self.theme.colors;
		let style = match segment.style {
			SegmentStyle::Normal => Style::default().fg(colors.ui.fg),
			SegmentStyle::Mode => colors.mode_style(&self.mode()).add_modifier(Modifier::BOLD),
			SegmentStyle::Inverted => Style::default().add_modifier(Modifier::REVERSED),
			SegmentStyle::Dim => Style::default().fg(colors.status.dim_fg),
			SegmentStyle::Warning => Style::default().fg(colors.status.warning_fg),
			SegmentStyle::Error => Style::default().fg(colors.status.error_fg),
			SegmentStyle::Success => Style::default().fg(colors.status.success_fg),
		};
		Span::styled(segment.text.clone(), style)
	}
}
