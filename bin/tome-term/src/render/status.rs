use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tome_core::Mode;
use tome_core::ext::{
	RenderedSegment, SegmentPosition, SegmentStyle, StatuslineContext, render_position,
};

use crate::editor::Editor;

impl Editor {
	pub fn render_status_line(&self) -> impl Widget + '_ {
		let ctx = StatuslineContext {
			mode_name: self.mode_name(),
			path: self
				.path
				.as_ref()
				.map(|p: &std::path::PathBuf| p.to_str().unwrap_or("[invalid path]")),
			modified: self.modified,
			line: self.cursor_line() + 1,
			col: self.cursor_col() + 1,
			count: self.input.count(),
			total_lines: self.doc.len_lines(),
			file_type: self.file_type.as_deref(),
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

	pub fn segment_to_span(&self, segment: &RenderedSegment) -> Span<'static> {
		let style = match segment.style {
			SegmentStyle::Normal => Style::default().fg(self.theme.colors.ui.fg),
			SegmentStyle::Mode => {
				let base = match self.mode() {
					Mode::Normal => Style::default()
						.bg(self.theme.colors.status.normal_bg)
						.fg(self.theme.colors.status.normal_fg),
					Mode::Insert => Style::default()
						.bg(self.theme.colors.status.insert_bg)
						.fg(self.theme.colors.status.insert_fg),
					Mode::Goto => Style::default()
						.bg(self.theme.colors.status.goto_bg)
						.fg(self.theme.colors.status.goto_fg),
					Mode::View => Style::default()
						.bg(self.theme.colors.status.view_bg)
						.fg(self.theme.colors.status.view_fg),
					Mode::Command { .. } => Style::default()
						.bg(self.theme.colors.status.command_bg)
						.fg(self.theme.colors.status.command_fg),
					Mode::PendingAction(_) => Style::default()
						.bg(self.theme.colors.status.command_bg)
						.fg(self.theme.colors.status.command_fg),
				};
				base.add_modifier(Modifier::BOLD)
			}
			SegmentStyle::Inverted => Style::default().add_modifier(Modifier::REVERSED),
			SegmentStyle::Dim => Style::default().fg(self.theme.colors.status.dim_fg),
			SegmentStyle::Warning => Style::default().fg(self.theme.colors.status.warning_fg),
			SegmentStyle::Error => Style::default().fg(self.theme.colors.status.error_fg),
			SegmentStyle::Success => Style::default().fg(self.theme.colors.status.success_fg),
		};
		Span::styled(segment.text.clone(), style)
	}
}
