//! Viewport scrolling and cursor visibility logic.

use evildoer_base::range::CharIdx;
use evildoer_tui::layout::Rect;
use serde::Serialize;
use tracing::debug;

use crate::buffer::Buffer;
use crate::render::types::{wrap_line, WrapSegment};

/// Test event emitted when viewport scrolling occurs.
#[derive(Serialize)]
struct ViewportEnsureEvent {
	/// Event type identifier.
	#[serde(rename = "type")]
	kind: &'static str,
	/// Action taken (scroll_up, scroll_down, suppress_scroll_down, etc.).
	action: &'static str,
	/// ID of the buffer being scrolled.
	buffer_id: u64,
	/// Current viewport height in lines.
	viewport_height: usize,
	/// Previous viewport height before resize.
	prev_viewport_height: usize,
	/// Line number at top of viewport.
	scroll_line: usize,
	/// Wrap segment at top of viewport.
	scroll_segment: usize,
	/// Line number of the cursor.
	cursor_line: usize,
	/// Wrap segment of the cursor.
	cursor_segment: usize,
	/// Whether the viewport is shrinking.
	viewport_shrinking: bool,
	/// Whether downward scrolling is suppressed.
	suppress_scroll_down: bool,
}

impl ViewportEnsureEvent {
	/// Logs a viewport event for testing purposes.
	fn log(
		action: &'static str,
		buffer: &Buffer,
		viewport_height: usize,
		prev_viewport_height: usize,
		cursor_line: usize,
		cursor_segment: usize,
		viewport_shrinking: bool,
	) {
		crate::test_events::write_test_event(&Self {
			kind: "viewport_ensure",
			action,
			buffer_id: buffer.id.0,
			viewport_height,
			prev_viewport_height,
			scroll_line: buffer.scroll_line,
			scroll_segment: buffer.scroll_segment,
			cursor_line,
			cursor_segment,
			viewport_shrinking,
			suppress_scroll_down: buffer.suppress_scroll_down,
		});
	}
}

/// Ensures the cursor is visible in the buffer's viewport.
///
/// This function adjusts `buffer.scroll_line` and `buffer.scroll_segment` to ensure
/// the primary cursor is visible within the given area. It also updates
/// `buffer.text_width` and `buffer.last_viewport_height` to match the current
/// rendering context.
///
/// # Viewport Shrink Behavior
///
/// When the viewport height shrinks (e.g., due to an adjacent split being resized),
/// this function will NOT scroll down to chase a cursor that went off-screen below.
/// This preserves the visual stability of the viewport's top edge during resize
/// operations. The cursor will become visible again when the user moves it or
/// when the viewport expands.
///
/// Scrolling UP to bring an off-screen cursor into view from above is always
/// performed, as this is typically intentional (cursor moved up, not resize).
pub fn ensure_buffer_cursor_visible(buffer: &mut Buffer, area: Rect) {
	let total_lines = buffer.doc().content.len_lines();
	let gutter_width = buffer.gutter_width();
	let text_width = area.width.saturating_sub(gutter_width) as usize;
	let viewport_height = area.height as usize;

	let cursor_pos: CharIdx = buffer.cursor;
	if cursor_pos != buffer.last_rendered_cursor {
		buffer.suppress_scroll_down = false;
	}

	let prev_viewport_height = buffer.last_viewport_height;

	// Detect if viewport is shrinking - used to avoid chasing cursor downward
	let viewport_shrinking = viewport_height < prev_viewport_height;

	// Debug: log viewport size changes
	if viewport_height != prev_viewport_height && prev_viewport_height > 0 {
		debug!(
			prev_height = prev_viewport_height,
			new_height = viewport_height,
			shrinking = viewport_shrinking,
			"Viewport height changed"
		);
	}

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
	);

	let cursor_line = buffer.cursor_line();
	let cursor_line_start: CharIdx = buffer.doc().content.line_to_char(cursor_line);
	let cursor_col = cursor_pos.saturating_sub(cursor_line_start);

	let cursor_line_end: CharIdx = if cursor_line + 1 < total_lines {
		buffer.doc().content.line_to_char(cursor_line + 1)
	} else {
		buffer.doc().content.len_chars()
	};
	let cursor_line_text: String = buffer
		.doc()
		.content
		.slice(cursor_line_start..cursor_line_end)
		.into();
	let cursor_line_text = cursor_line_text.trim_end_matches('\n');
	let cursor_segments = wrap_line(cursor_line_text, text_width);
	let cursor_segment = find_segment_for_col(&cursor_segments, cursor_col);

	// Cursor is above viewport - always scroll up to show it
	if cursor_line < buffer.scroll_line
		|| (cursor_line == buffer.scroll_line && cursor_segment < buffer.scroll_segment)
	{
		buffer.scroll_line = cursor_line;
		buffer.scroll_segment = cursor_segment;
		buffer.suppress_scroll_down = false;
		ViewportEnsureEvent::log(
			"scroll_up",
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

	let mut cursor_visible = cursor_visible_from(
		buffer,
		buffer.scroll_line,
		buffer.scroll_segment,
		cursor_line,
		cursor_segment,
		viewport_height,
		text_width,
	);

	if cursor_visible {
		buffer.suppress_scroll_down = false;
		buffer.last_rendered_cursor = cursor_pos;
		return;
	}

	// If viewport is shrinking, don't chase cursor downward.
	// This preserves the visual position of the viewport's top edge during
	// resize operations. The cursor may temporarily go off-screen below,
	// but will reappear when the user moves it or the viewport expands.
	if viewport_shrinking {
		buffer.suppress_scroll_down = true;
		debug!(
			scroll_line = buffer.scroll_line,
			cursor_line = cursor_line,
			"Viewport shrinking - NOT scrolling to chase cursor"
		);
		ViewportEnsureEvent::log(
			"suppress_scroll_down",
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

	if buffer.suppress_scroll_down {
		ViewportEnsureEvent::log(
			"skip_scroll_down",
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

	// Scroll down until cursor is visible
	let original_scroll = buffer.scroll_line;
	let mut prev_scroll = (buffer.scroll_line, buffer.scroll_segment);
	while !cursor_visible {
		scroll_viewport_down(buffer, text_width);

		let new_scroll = (buffer.scroll_line, buffer.scroll_segment);
		if new_scroll == prev_scroll {
			break;
		}
		prev_scroll = new_scroll;

		cursor_visible = cursor_visible_from(
			buffer,
			buffer.scroll_line,
			buffer.scroll_segment,
			cursor_line,
			cursor_segment,
			viewport_height,
			text_width,
		);
	}

	if buffer.scroll_line != original_scroll {
		debug!(
			from = original_scroll,
			to = buffer.scroll_line,
			cursor_line = cursor_line,
			viewport_height = viewport_height,
			"Scrolled down to chase cursor"
		);
		ViewportEnsureEvent::log(
			"scroll_down",
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

/// Clamps a segment index to valid range for a given line.
fn clamp_segment_for_line(
	buffer: &Buffer,
	line: usize,
	segment: usize,
	text_width: usize,
) -> usize {
	let total_lines = buffer.doc().content.len_lines();
	if line >= total_lines {
		return 0;
	}

	let line_start: CharIdx = buffer.doc().content.line_to_char(line);
	let line_end: CharIdx = if line + 1 < total_lines {
		buffer.doc().content.line_to_char(line + 1)
	} else {
		buffer.doc().content.len_chars()
	};

	let line_text: String = buffer.doc().content.slice(line_start..line_end).into();
	let line_text = line_text.trim_end_matches('\n');
	let segments = wrap_line(line_text, text_width);
	let num_segments = segments.len().max(1);

	segment.min(num_segments.saturating_sub(1))
}

/// Finds which wrap segment contains the given column.
fn find_segment_for_col(segments: &[WrapSegment], col: usize) -> usize {
	for (i, seg) in segments.iter().enumerate() {
		let seg_end = seg.start_offset + seg.text.chars().count();
		if col < seg_end {
			return i;
		}
	}
	segments.len().saturating_sub(1)
}

/// Checks if the cursor is visible from a given viewport start position.
fn cursor_visible_from(
	buffer: &Buffer,
	start_line: usize,
	start_segment: usize,
	cursor_line: usize,
	cursor_segment: usize,
	viewport_height: usize,
	text_width: usize,
) -> bool {
	if viewport_height == 0 {
		return false;
	}

	let total_lines = buffer.doc().content.len_lines();
	if start_line >= total_lines {
		return false;
	}

	let mut line = start_line;
	let mut segment = clamp_segment_for_line(buffer, line, start_segment, text_width);

	for _ in 0..viewport_height {
		if line == cursor_line && segment == cursor_segment {
			return true;
		}

		if !advance_one_visual_row(buffer, &mut line, &mut segment, text_width) {
			break;
		}
	}

	false
}

/// Advances the viewport position by one visual row.
fn advance_one_visual_row(
	buffer: &Buffer,
	line: &mut usize,
	segment: &mut usize,
	text_width: usize,
) -> bool {
	let total_lines = buffer.doc().content.len_lines();
	if *line >= total_lines {
		return false;
	}

	let line_start: CharIdx = buffer.doc().content.line_to_char(*line);
	let line_end: CharIdx = if *line + 1 < total_lines {
		buffer.doc().content.line_to_char(*line + 1)
	} else {
		buffer.doc().content.len_chars()
	};

	let line_text: String = buffer.doc().content.slice(line_start..line_end).into();
	let line_text = line_text.trim_end_matches('\n');
	let segments = wrap_line(line_text, text_width);
	let num_segments = segments.len().max(1);

	if *segment + 1 < num_segments {
		*segment += 1;
		return true;
	}

	if *line + 1 < total_lines {
		*line += 1;
		*segment = 0;
		return true;
	}

	false
}

/// Scrolls viewport down by one visual line.
fn scroll_viewport_down(buffer: &mut Buffer, text_width: usize) {
	let total_lines = buffer.doc().content.len_lines();
	if buffer.scroll_line >= total_lines {
		return;
	}

	let line_start: CharIdx = buffer.doc().content.line_to_char(buffer.scroll_line);
	let line_end: CharIdx = if buffer.scroll_line + 1 < total_lines {
		buffer.doc().content.line_to_char(buffer.scroll_line + 1)
	} else {
		buffer.doc().content.len_chars()
	};

	let line_text: String = buffer.doc().content.slice(line_start..line_end).into();
	let line_text = line_text.trim_end_matches('\n');
	let segments = wrap_line(line_text, text_width);
	let num_segments = segments.len().max(1);

	if buffer.scroll_segment + 1 < num_segments {
		buffer.scroll_segment += 1;
	} else if buffer.scroll_line + 1 < total_lines {
		buffer.scroll_line += 1;
		buffer.scroll_segment = 0;
	}
}
