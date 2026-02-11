use xeno_primitives::visible_line_count;
use xeno_registry::statusline::{
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

		let (sync_role_str, sync_status_str): (Option<&str>, Option<&str>) = (None, None);

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
		let mut current_width = 0usize;
		let mut all_segments = Vec::new();

		for seg in render_position(SegmentPosition::Left, &ctx) {
			all_segments.push(seg);
		}
		for seg in render_position(SegmentPosition::Center, &ctx) {
			all_segments.push(seg);
		}
		for seg in render_position(SegmentPosition::Right, &ctx) {
			all_segments.push(seg);
		}

		let mut mode_segments = Vec::new();
		let mut body_segments = Vec::new();
		for seg in all_segments {
			if matches!(seg.style, SegmentStyle::Mode) {
				mode_segments.push(seg);
			} else {
				body_segments.push(seg);
			}
		}

		let mode_width: usize = mode_segments
			.iter()
			.map(|seg| crate::render::cell_width(&seg.text))
			.sum();

		for seg in body_segments {
			current_width += crate::render::cell_width(&seg.text);
			spans.push(self.segment_to_span(&seg));
		}

		if let Some(active) = self.state.overlay_system.interaction.active.as_ref() {
			let label = match active.controller.name() {
				"CommandPalette" => "Cmd",
				"Search" => "Search",
				other => other,
			};
			let tag = format!(" [{label}]");
			let viewport_width = self.state.viewport.width.unwrap_or(0) as usize;
			let tag_width = crate::render::cell_width(&tag);
			if viewport_width > 0 && current_width + tag_width + mode_width <= viewport_width {
				spans.push(Span::styled(
					tag,
					Style::default().fg(self.state.config.theme.colors.semantic.dim),
				));
				current_width += tag_width;
			}
		}

		let viewport_width = self.state.viewport.width.unwrap_or(0) as usize;
		if viewport_width > 0 && mode_width > 0 && current_width + mode_width < viewport_width {
			spans.push(Span::raw(
				" ".repeat(viewport_width.saturating_sub(current_width + mode_width)),
			));
		}

		for seg in mode_segments {
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
