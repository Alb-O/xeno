use evildoer_manifest::{
	Mode, RenderedSegment, SegmentPosition, SegmentStyle, StatuslineContext, render_position,
};
use evildoer_tui::style::{Modifier, Style};
use evildoer_tui::text::{Line, Span};
use evildoer_tui::widgets::{Paragraph, Widget};

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

		// Extract data before creating the context to avoid lifetime issues
		let (path_str, file_type_str, modified, mode_name, line, col, count, total_lines) =
			if self.is_terminal_focused() {
				(
					None,
					Some("terminal".to_string()),
					false,
					"TERMINAL",
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
