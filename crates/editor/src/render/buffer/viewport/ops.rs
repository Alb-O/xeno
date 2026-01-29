use xeno_primitives::range::CharIdx;
use xeno_primitives::visible_line_count;
use xeno_tui::layout::Rect;

use super::types::ViewportEnsureEvent;
use crate::buffer::{Buffer, Document};
use crate::render::wrap::{WrappedSegment, wrap_line_ranges_rope};

/// Ensures the cursor is visible in the buffer's viewport with scroll margins.
///
/// This function adjusts `buffer.scroll_line` and `buffer.scroll_segment` to ensure
/// the primary cursor is visible within the given area, maintaining a minimum
/// distance from the viewport edges when possible.
pub fn ensure_buffer_cursor_visible(
	buffer: &mut Buffer,
	area: Rect,
	tab_width: usize,
	scroll_margin: usize,
) {
	let total_lines = buffer.with_doc(|doc: &Document| visible_line_count(doc.content().slice(..)));
	let gutter_width = buffer.gutter_width();
	let text_width = area.width.saturating_sub(gutter_width) as usize;
	let viewport_height = area.height as usize;

	let cursor_pos: CharIdx = buffer.cursor;
	if cursor_pos != buffer.last_rendered_cursor {
		buffer.suppress_auto_scroll = false;
	}

	let prev_viewport_height = buffer.last_viewport_height;
	let viewport_shrinking = viewport_height < prev_viewport_height;

	buffer.text_width = text_width;
	buffer.last_viewport_height = viewport_height;

	if buffer.scroll_line >= total_lines {
		buffer.scroll_line = total_lines.saturating_sub(1);
		buffer.scroll_segment = 0;
	}
	buffer.scroll_segment = clamp_segment_for_line(
		buffer,
		buffer.scroll_line,
		buffer.scroll_segment,
		text_width,
		tab_width,
	);

	let cursor_line = buffer.cursor_line();
	let (cursor_col, cursor_segments) = buffer.with_doc(|doc: &Document| {
		let start = doc.content().line_to_char(cursor_line);
		let col = cursor_pos.saturating_sub(start);
		let line_slice = doc.content().line(cursor_line);
		let line_len = line_slice.len_chars();
		let has_newline = line_len > 0 && line_slice.char(line_len - 1) == '\n';
		let content = if has_newline {
			line_slice.slice(..line_len - 1)
		} else {
			line_slice
		};
		let segments = wrap_line_ranges_rope(content, text_width, tab_width);
		(col, segments)
	});
	let cursor_segment = find_segment_for_col(&cursor_segments, cursor_col);

	let effective_margin = scroll_margin.min(viewport_height.saturating_sub(1) / 2);
	let min_row = effective_margin;
	let max_row = viewport_height
		.saturating_sub(1)
		.saturating_sub(effective_margin);

	let cursor_row = cursor_row_in_viewport(
		buffer,
		buffer.scroll_line,
		buffer.scroll_segment,
		cursor_line,
		cursor_segment,
		viewport_height,
		text_width,
		tab_width,
	);

	let needs_scroll_up = match cursor_row {
		None => {
			cursor_line < buffer.scroll_line
				|| (cursor_line == buffer.scroll_line && cursor_segment < buffer.scroll_segment)
		}
		Some(row) => row < min_row && buffer.scroll_line > 0,
	};

	let needs_scroll_down = match cursor_row {
		None => !needs_scroll_up,
		Some(row) => row > max_row && cursor_line + 1 < total_lines,
	};

	if buffer.suppress_auto_scroll && (needs_scroll_up || needs_scroll_down) {
		ViewportEnsureEvent::log(
			"suppress_auto_scroll",
			buffer,
			viewport_height,
			prev_viewport_height,
			cursor_line,
			cursor_segment,
			viewport_shrinking,
		);
		buffer.last_rendered_cursor = cursor_pos;
		return;
	}

	if needs_scroll_down && viewport_shrinking {
		ViewportEnsureEvent::log(
			"skip_scroll_on_shrink",
			buffer,
			viewport_height,
			prev_viewport_height,
			cursor_line,
			cursor_segment,
			viewport_shrinking,
		);
		buffer.last_rendered_cursor = cursor_pos;
		return;
	}

	let original_scroll = (buffer.scroll_line, buffer.scroll_segment);

	let target_row = if needs_scroll_up {
		Some(min_row)
	} else if needs_scroll_down {
		Some(max_row)
	} else {
		None
	};

	if let Some(row) = target_row {
		let (new_line, new_seg) = scroll_position_for_cursor_at_row(
			buffer,
			cursor_line,
			cursor_segment,
			row,
			text_width,
			tab_width,
		);
		buffer.scroll_line = new_line;
		buffer.scroll_segment = new_seg;
		buffer.suppress_auto_scroll = false;
	}

	let new_scroll = (buffer.scroll_line, buffer.scroll_segment);
	if new_scroll != original_scroll {
		let action = if needs_scroll_up {
			"scroll_up"
		} else {
			"scroll_down"
		};
		ViewportEnsureEvent::log(
			action,
			buffer,
			viewport_height,
			prev_viewport_height,
			cursor_line,
			cursor_segment,
			viewport_shrinking,
		);
	}

	buffer.last_rendered_cursor = cursor_pos;
}

/// Computes scroll position to place cursor at a specific visual row.
fn scroll_position_for_cursor_at_row(
	buffer: &Buffer,
	cursor_line: usize,
	cursor_segment: usize,
	target_row: usize,
	text_width: usize,
	tab_width: usize,
) -> (usize, usize) {
	let mut line = cursor_line;
	let mut segment = cursor_segment;
	let mut rows_above = 0;

	while rows_above < target_row && (line > 0 || segment > 0) {
		if segment > 0 {
			segment -= 1;
		} else {
			line -= 1;
			segment = line_segment_count(buffer, line, text_width, tab_width).saturating_sub(1);
		}
		rows_above += 1;
	}

	(line, segment)
}

/// Returns the number of wrap segments for a line.
fn line_segment_count(buffer: &Buffer, line: usize, text_width: usize, tab_width: usize) -> usize {
	buffer.with_doc(|doc: &Document| {
		let total_lines = doc.content().len_lines();
		if line >= total_lines {
			return 1;
		}

		let line_slice = doc.content().line(line);
		let line_len = line_slice.len_chars();
		let has_newline = line_len > 0 && line_slice.char(line_len - 1) == '\n';
		let content = if has_newline {
			line_slice.slice(..line_len - 1)
		} else {
			line_slice
		};
		wrap_line_ranges_rope(content, text_width, tab_width)
			.len()
			.max(1)
	})
}

/// Clamps a segment index to valid range for a given line.
fn clamp_segment_for_line(
	buffer: &Buffer,
	line: usize,
	segment: usize,
	text_width: usize,
	tab_width: usize,
) -> usize {
	buffer.with_doc(|doc: &Document| {
		let total_lines = doc.content().len_lines();
		if line >= total_lines {
			return 0;
		}

		let line_slice = doc.content().line(line);
		let line_len = line_slice.len_chars();
		let has_newline = line_len > 0 && line_slice.char(line_len - 1) == '\n';
		let content = if has_newline {
			line_slice.slice(..line_len - 1)
		} else {
			line_slice
		};
		let segments = wrap_line_ranges_rope(content, text_width, tab_width);
		let num_segments = segments.len().max(1);

		segment.min(num_segments.saturating_sub(1))
	})
}

/// Finds which wrap segment contains the given column.
fn find_segment_for_col(segments: &[WrappedSegment], col: usize) -> usize {
	for (i, seg) in segments.iter().enumerate() {
		let seg_end = seg.start_char_offset + seg.char_len;
		if col < seg_end {
			return i;
		}
	}
	segments.len().saturating_sub(1)
}

/// Returns the cursor's visual row within the viewport (0-indexed), or None if not visible.
fn cursor_row_in_viewport(
	buffer: &Buffer,
	start_line: usize,
	start_segment: usize,
	cursor_line: usize,
	cursor_segment: usize,
	viewport_height: usize,
	text_width: usize,
	tab_width: usize,
) -> Option<usize> {
	if viewport_height == 0 {
		return None;
	}

	let total_lines = buffer.with_doc(|doc: &Document| visible_line_count(doc.content().slice(..)));
	if start_line >= total_lines {
		return None;
	}

	let mut line = start_line;
	let mut segment = clamp_segment_for_line(buffer, line, start_segment, text_width, tab_width);

	for row in 0..viewport_height {
		if line == cursor_line && segment == cursor_segment {
			return Some(row);
		}

		if !advance_one_visual_row(buffer, &mut line, &mut segment, text_width, tab_width) {
			break;
		}
	}

	None
}

/// Advances the viewport position by one visual row.
fn advance_one_visual_row(
	buffer: &Buffer,
	line: &mut usize,
	segment: &mut usize,
	text_width: usize,
	tab_width: usize,
) -> bool {
	let (visible_lines, num_segments) = buffer.with_doc(|doc: &Document| {
		let content = doc.content();
		let visible = visible_line_count(content.slice(..));
		if *line >= visible {
			return (visible, 0);
		}

		let line_slice = content.line(*line);
		let line_len = line_slice.len_chars();
		let has_newline = line_len > 0 && line_slice.char(line_len - 1) == '\n';
		let content = if has_newline {
			line_slice.slice(..line_len - 1)
		} else {
			line_slice
		};
		let n = wrap_line_ranges_rope(content, text_width, tab_width)
			.len()
			.max(1);
		(visible, n)
	});

	if *line >= visible_lines {
		return false;
	}

	if *segment + 1 < num_segments {
		*segment += 1;
		return true;
	}

	if *line + 1 < visible_lines {
		*line += 1;
		*segment = 0;
		return true;
	}

	false
}
