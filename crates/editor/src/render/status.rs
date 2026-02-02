use xeno_primitives::visible_line_count;
use xeno_registry::{
	RenderedSegment, SegmentPosition, SegmentStyle, StatuslineContext, render_position,
};
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::{Paragraph, Widget};

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
		let buffer = self.buffer();
		let path_str: Option<String> = buffer
			.path()
			.as_ref()
			.and_then(|p| p.to_str().map(|s| s.to_string()));
		let file_type_str: Option<String> = buffer.file_type();
		let modified = buffer.modified();
		let readonly = buffer.is_readonly();
		let count = buffer.input.count();
		let total_lines = buffer.with_doc(|doc| visible_line_count(doc.content().slice(..)));
		let mode_name = self.mode_name();
		let line = self.cursor_line() + 1;
		let col = self.cursor_col() + 1;

		#[cfg(feature = "lsp")]
		let (sync_role_str, sync_status_str) = {
			if let Some(uri) = buffer
				.path()
				.as_ref()
				.and_then(|p| xeno_lsp::uri_from_path(p))
			{
				let (role, status) = self.state.buffer_sync.ui_status_for_uri(uri.as_str());
				let role_s = match role {
					Some(xeno_broker_proto::types::BufferSyncRole::Owner) => "Owner",
					Some(xeno_broker_proto::types::BufferSyncRole::Follower) => "Follower",
					None => "None",
				};
				let status_s = match status {
					crate::buffer_sync::SyncStatus::Off => "Off",
					crate::buffer_sync::SyncStatus::Owner => "O",
					crate::buffer_sync::SyncStatus::Follower => "F",
					crate::buffer_sync::SyncStatus::Acquiring => "Acq",
					crate::buffer_sync::SyncStatus::Confirming => "Conf",
					crate::buffer_sync::SyncStatus::NeedsResync => "Sync!",
				};
				(Some(role_s), Some(status_s))
			} else {
				(None, None)
			}
		};
		#[cfg(not(feature = "lsp"))]
		let (sync_role_str, sync_status_str) = (None, None);

		let ctx = StatuslineContext {
			mode_name,
			path: path_str.as_deref(),
			modified,
			readonly,
			line,
			col,
			count,
			total_lines,
			file_type: file_type_str.as_deref(),
			buffer_index,
			buffer_count,
			sync_role: sync_role_str,
			sync_status: sync_status_str,
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
		let colors = &self.state.config.theme.colors;
		let style = match segment.style {
			SegmentStyle::Normal => Style::default().fg(colors.ui.fg),
			SegmentStyle::Mode => colors.mode_style(&self.mode()).add_modifier(Modifier::BOLD),
			SegmentStyle::Inverted => Style::default().add_modifier(Modifier::REVERSED),
			SegmentStyle::Dim => Style::default().fg(colors.semantic.dim),
			SegmentStyle::Warning => Style::default().fg(colors.semantic.warning),
			SegmentStyle::Error => Style::default().fg(colors.semantic.error),
			SegmentStyle::Success => Style::default().fg(colors.semantic.success),
		};
		Span::styled(segment.text.clone(), style)
	}
}
