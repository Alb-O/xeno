//! Viewport scrolling and cursor visibility logic.

use serde::Serialize;
use tracing::{debug, trace};
use xeno_primitives::range::CharIdx;
use xeno_tui::layout::Rect;

use crate::buffer::Buffer;
use crate::render::wrap::{WrapSegment, wrap_line};

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

/// Ensures the cursor is visible in the buffer's viewport with scroll margins.
///
/// This function adjusts `buffer.scroll_line` and `buffer.scroll_segment` to ensure
/// the primary cursor is visible within the given area, maintaining a minimum
/// distance from the viewport edges when possible.
///
/// # Scroll Margin
///
/// The `scroll_margin` parameter specifies the preferred minimum lines between
/// the cursor and viewport edges. When the cursor moves within this zone, the
/// viewport scrolls to restore the margin. At buffer boundaries (first/last line),
/// the cursor is allowed to reach the edge since scrolling further is impossible.
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
///
/// # Parameters
/// - `buffer`: The buffer to ensure cursor visibility for
/// - `area`: The rectangular area the buffer is rendered into
/// - `tab_width`: Number of spaces a tab character occupies (from options)
/// - `scroll_margin`: Preferred minimum lines above/below cursor
pub fn ensure_buffer_cursor_visible(
	buffer: &mut Buffer,
	area: Rect,
	tab_width: usize,
	scroll_margin: usize,
) {
	let total_lines = buffer.doc().content().len_lines();
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
		tab_width,
	);

	let cursor_line = buffer.cursor_line();
	let cursor_line_start: CharIdx = buffer.doc().content().line_to_char(cursor_line);
	let cursor_col = cursor_pos.saturating_sub(cursor_line_start);

	let cursor_line_end: CharIdx = if cursor_line + 1 < total_lines {
		buffer.doc().content().line_to_char(cursor_line + 1)
	} else {
		buffer.doc().content().len_chars()
	};
	let cursor_line_text: String = buffer
		.doc()
		.content()
		.slice(cursor_line_start..cursor_line_end)
		.into();
	let cursor_line_text = cursor_line_text.trim_end_matches('\n');
	let cursor_segments = wrap_line(cursor_line_text, text_width, tab_width);
	let cursor_segment = find_segment_for_col(&cursor_segments, cursor_col);

	let effective_margin = scroll_margin.min(viewport_height.saturating_sub(1) / 2);
	let min_row = effective_margin; // cursor should be at least this far from top
	let max_row = viewport_height
		.saturating_sub(1)
		.saturating_sub(effective_margin);

	// Find cursor's current visual row in viewport (None if not visible)
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

	// Determine if scroll adjustment is needed
	let needs_scroll_up = match cursor_row {
		None => {
			cursor_line < buffer.scroll_line
				|| (cursor_line == buffer.scroll_line && cursor_segment < buffer.scroll_segment)
		}
		Some(row) => row < min_row && buffer.scroll_line > 0,
	};

	let needs_scroll_down = match cursor_row {
		None => !needs_scroll_up, // cursor below viewport
		Some(row) => row > max_row && cursor_line + 1 < total_lines,
	};

	// Handle viewport shrinking - don't chase cursor downward
	if needs_scroll_down && (viewport_shrinking || buffer.suppress_scroll_down) {
		if viewport_shrinking {
			buffer.suppress_scroll_down = true;
		}
		ViewportEnsureEvent::log(
			if viewport_shrinking {
				"suppress_scroll_down"
			} else {
				"skip_scroll_down"
			},
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

	if needs_scroll_up {
		// Scroll up: put cursor at min_row
		let (new_line, new_seg) = scroll_position_for_cursor_at_row(
			buffer,
			cursor_line,
			cursor_segment,
			min_row,
			text_width,
			tab_width,
		);
		buffer.scroll_line = new_line;
		buffer.scroll_segment = new_seg;
		buffer.suppress_scroll_down = false;
	} else if needs_scroll_down {
		// Scroll down: put cursor at max_row
		let (new_line, new_seg) = scroll_position_for_cursor_at_row(
			buffer,
			cursor_line,
			cursor_segment,
			max_row,
			text_width,
			tab_width,
		);
		buffer.scroll_line = new_line;
		buffer.scroll_segment = new_seg;
		buffer.suppress_scroll_down = false;
	}

	let new_scroll = (buffer.scroll_line, buffer.scroll_segment);
	if new_scroll != original_scroll {
		let action = if needs_scroll_up {
			"scroll_up"
		} else {
			"scroll_down"
		};
		trace!(
			from = original_scroll.0,
			to = buffer.scroll_line,
			cursor_line = cursor_line,
			viewport_height = viewport_height,
			action,
			"Scrolled to maintain margin"
		);
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
	let total_lines = buffer.doc().content().len_lines();
	if line >= total_lines {
		return 1;
	}

	let line_start: CharIdx = buffer.doc().content().line_to_char(line);
	let line_end: CharIdx = if line + 1 < total_lines {
		buffer.doc().content().line_to_char(line + 1)
	} else {
		buffer.doc().content().len_chars()
	};

	let line_text: String = buffer.doc().content().slice(line_start..line_end).into();
	let line_text = line_text.trim_end_matches('\n');
	wrap_line(line_text, text_width, tab_width).len().max(1)
}

/// Clamps a segment index to valid range for a given line.
fn clamp_segment_for_line(
	buffer: &Buffer,
	line: usize,
	segment: usize,
	text_width: usize,
	tab_width: usize,
) -> usize {
	let total_lines = buffer.doc().content().len_lines();
	if line >= total_lines {
		return 0;
	}

	let line_start: CharIdx = buffer.doc().content().line_to_char(line);
	let line_end: CharIdx = if line + 1 < total_lines {
		buffer.doc().content().line_to_char(line + 1)
	} else {
		buffer.doc().content().len_chars()
	};

	let line_text: String = buffer.doc().content().slice(line_start..line_end).into();
	let line_text = line_text.trim_end_matches('\n');
	let segments = wrap_line(line_text, text_width, tab_width);
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

	let total_lines = buffer.doc().content().len_lines();
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
	let total_lines = buffer.doc().content().len_lines();
	if *line >= total_lines {
		return false;
	}

	let line_start: CharIdx = buffer.doc().content().line_to_char(*line);
	let line_end: CharIdx = if *line + 1 < total_lines {
		buffer.doc().content().line_to_char(*line + 1)
	} else {
		buffer.doc().content().len_chars()
	};

	let line_text: String = buffer.doc().content().slice(line_start..line_end).into();
	let line_text = line_text.trim_end_matches('\n');
	let segments = wrap_line(line_text, text_width, tab_width);
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
