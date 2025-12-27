use tome_tui::style::{Modifier, Style};
use tome_tui::text::{Line, Span};
use tome_tui::widgets::{Paragraph, Widget};
use tome_manifest::{
	Mode, RenderedSegment, SegmentPosition, SegmentStyle, StatuslineContext, render_position,
};

use crate::Editor;

impl Editor {
	pub fn render_status_line(&self) -> impl Widget + '_ {
		let buffer_ids = self.buffer_ids();
		let buffer_index = self
			.focused_buffer_id()
			.and_then(|current_id| buffer_ids.iter().position(|&id| id == current_id))
			.unwrap_or(0)
			+ 1;
		let buffer_count = buffer_ids.len();

		// When a terminal is focused, show terminal-specific status
		let ctx = if self.is_terminal_focused() {
			StatuslineContext {
				mode_name: "TERMINAL",
				path: None,
				modified: false,
				line: 0,
				col: 0,
				count: 0,
				total_lines: 0,
				file_type: Some("terminal"),
				buffer_index,
				buffer_count,
			}
		} else {
			let buffer = self.buffer();
			StatuslineContext {
				mode_name: self.mode_name(),
				path: buffer
					.path
					.as_ref()
					.map(|p: &std::path::PathBuf| p.to_str().unwrap_or("[invalid path]")),
				modified: buffer.modified,
				line: self.cursor_line() + 1,
				col: self.cursor_col() + 1,
				count: buffer.input.count(),
				total_lines: buffer.doc.len_lines(),
				file_type: buffer.file_type.as_deref(),
				buffer_index,
				buffer_count,
			}
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
			SegmentStyle::Normal => Style::default().fg(self.theme.colors.ui.fg.into()),
			SegmentStyle::Mode => {
				let base = match self.mode() {
					Mode::Normal => Style::default()
						.bg(self.theme.colors.status.normal_bg.into())
						.fg(self.theme.colors.status.normal_fg.into()),
					Mode::Insert => Style::default()
						.bg(self.theme.colors.status.insert_bg.into())
						.fg(self.theme.colors.status.insert_fg.into()),
					Mode::Goto => Style::default()
						.bg(self.theme.colors.status.goto_bg.into())
						.fg(self.theme.colors.status.goto_fg.into()),
					Mode::View => Style::default()
						.bg(self.theme.colors.status.view_bg.into())
						.fg(self.theme.colors.status.view_fg.into()),
					Mode::Window => Style::default()
						.bg(self.theme.colors.status.goto_bg.into())
						.fg(self.theme.colors.status.goto_fg.into()),
					Mode::PendingAction(_) => Style::default()
						.bg(self.theme.colors.status.command_bg.into())
						.fg(self.theme.colors.status.command_fg.into()),
				};
				base.add_modifier(Modifier::BOLD)
			}
			SegmentStyle::Inverted => Style::default().add_modifier(Modifier::REVERSED),
			SegmentStyle::Dim => Style::default().fg(self.theme.colors.status.dim_fg.into()),
			SegmentStyle::Warning => {
				Style::default().fg(self.theme.colors.status.warning_fg.into())
			}
			SegmentStyle::Error => Style::default().fg(self.theme.colors.status.error_fg.into()),
			SegmentStyle::Success => {
				Style::default().fg(self.theme.colors.status.success_fg.into())
			}
		};
		Span::styled(segment.text.clone(), style)
	}
}
