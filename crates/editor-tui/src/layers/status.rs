use xeno_editor::Editor;
use xeno_registry::statusline::{RenderedSegment, SegmentPosition, SegmentStyle, StatuslineContext, render_position};
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::{Block, Paragraph};

use crate::text_width::cell_width;

fn segment_to_span(ed: &Editor, segment: &RenderedSegment) -> Span<'static> {
	let colors = &ed.config().theme.colors;
	let style = match segment.style {
		SegmentStyle::Normal => Style::default().fg(colors.ui.fg),
		SegmentStyle::Mode => colors.mode_style(&ed.mode()).add_modifier(Modifier::BOLD),
		SegmentStyle::Inverted => Style::default().add_modifier(Modifier::REVERSED),
		SegmentStyle::Dim => Style::default().fg(colors.semantic.dim),
		SegmentStyle::Warning => Style::default().fg(colors.semantic.warning),
		SegmentStyle::Error => Style::default().fg(colors.semantic.error),
		SegmentStyle::Success => Style::default().fg(colors.semantic.success),
	};
	Span::styled(segment.text.clone(), style)
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame, area: Rect) {
	let status_bg = Block::default().style(Style::default().bg(ed.config().theme.colors.ui.bg));
	frame.render_widget(status_bg, area);

	let buffer_ids = ed.buffer_ids();
	let buffer_index = ed
		.focused_buffer_id()
		.and_then(|current_id| buffer_ids.iter().position(|&id| id == current_id))
		.unwrap_or(0)
		+ 1;
	let buffer_count = buffer_ids.len();

	let buffer = ed.buffer();
	let path_str: Option<String> = buffer.path().as_ref().and_then(|p| p.to_str().map(|s| s.to_string()));
	let file_type_str: Option<String> = buffer.file_type();
	let modified = buffer.modified();
	let readonly = buffer.is_readonly();
	let count = buffer.input.count();
	let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
	let mode_name = ed.mode_name();
	let line = ed.cursor_line() + 1;
	let col = ed.cursor_col() + 1;

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

	let mode_width: usize = mode_segments.iter().map(|seg| cell_width(&seg.text)).sum();

	for seg in body_segments {
		current_width += cell_width(&seg.text);
		spans.push(segment_to_span(ed, &seg));
	}

	if let Some(label) = ed.status_overlay_label() {
		let tag = format!(" [{label}]");
		let viewport_width = ed.viewport().width.unwrap_or(0) as usize;
		let tag_width = cell_width(&tag);
		if viewport_width > 0 && current_width + tag_width + mode_width <= viewport_width {
			spans.push(Span::styled(tag, Style::default().fg(ed.config().theme.colors.semantic.dim)));
			current_width += tag_width;
		}
	}

	let viewport_width = ed.viewport().width.unwrap_or(0) as usize;
	if viewport_width > 0 && mode_width > 0 && current_width + mode_width < viewport_width {
		spans.push(Span::raw(" ".repeat(viewport_width.saturating_sub(current_width + mode_width))));
	}

	for seg in mode_segments {
		spans.push(segment_to_span(ed, &seg));
	}

	frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
