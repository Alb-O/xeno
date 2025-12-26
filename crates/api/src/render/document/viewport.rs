use ratatui::layout::Rect;
use tome_base::range::CharIdx;

use crate::Editor;

impl Editor {
	/// Clamps a segment index to valid range for a given line.
	///
	/// Ensures the segment index is within bounds for the wrapped segments of the
	/// specified line. This prevents invalid segment indices when scrolling or
	/// moving the viewport.
	///
	/// # Parameters
	/// - `line`: The line index in the document
	/// - `segment`: The desired segment index within the line
	/// - `text_width`: Available width for text rendering
	///
	/// # Returns
	/// A valid segment index clamped to [0, num_segments-1]. Returns 0 if the
	/// line is out of bounds.
	fn clamp_segment_for_line(&self, line: usize, segment: usize, text_width: usize) -> usize {
		let total_lines = self.buffer().doc.len_lines();
		if line >= total_lines {
			return 0;
		}

		let line_start: CharIdx = self.buffer().doc.line_to_char(line);
		let line_end: CharIdx = if line + 1 < total_lines {
			self.buffer().doc.line_to_char(line + 1)
		} else {
			self.buffer().doc.len_chars()
		};

		let line_text: String = self.buffer().doc.slice(line_start..line_end).into();
		let line_text = line_text.trim_end_matches('\n');
		let segments = self.wrap_line(line_text, text_width);
		let num_segments = segments.len().max(1);

		segment.min(num_segments.saturating_sub(1))
	}

	/// Advances the viewport position by one visual row.
	///
	/// Moves to the next wrapped segment within the current line, or to the first
	/// segment of the next line if at the end of the current line.
	///
	/// # Parameters
	/// - `line`: Mutable reference to the current line index
	/// - `segment`: Mutable reference to the current segment index
	/// - `text_width`: Available width for text rendering
	///
	/// # Returns
	/// `true` if successfully advanced, `false` if already at the end of the document.
	fn advance_one_visual_row(
		&self,
		line: &mut usize,
		segment: &mut usize,
		text_width: usize,
	) -> bool {
		let total_lines = self.buffer().doc.len_lines();
		if *line >= total_lines {
			return false;
		}

		let line_start: CharIdx = self.buffer().doc.line_to_char(*line);
		let line_end: CharIdx = if *line + 1 < total_lines {
			self.buffer().doc.line_to_char(*line + 1)
		} else {
			self.buffer().doc.len_chars()
		};

		let line_text: String = self.buffer().doc.slice(line_start..line_end).into();
		let line_text = line_text.trim_end_matches('\n');
		let segments = self.wrap_line(line_text, text_width);
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

	/// Checks if the cursor is visible from a given viewport start position.
	///
	/// Simulates forward iteration through visual rows to determine if the cursor
	/// position would be visible within the viewport height.
	///
	/// # Parameters
	/// - `start_line`: Starting line of the viewport
	/// - `start_segment`: Starting segment within the starting line
	/// - `cursor_line`: Line containing the cursor
	/// - `cursor_segment`: Segment within the cursor line
	/// - `viewport_height`: Height of the viewport in rows
	/// - `text_width`: Available width for text rendering
	///
	/// # Returns
	/// `true` if the cursor would be visible, `false` otherwise.
	fn cursor_visible_from(
		&self,
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

		let total_lines = self.buffer().doc.len_lines();
		if start_line >= total_lines {
			return false;
		}

		let mut line = start_line;
		let mut segment = self.clamp_segment_for_line(line, start_segment, text_width);

		for _ in 0..viewport_height {
			if line == cursor_line && segment == cursor_segment {
				return true;
			}

			if !self.advance_one_visual_row(&mut line, &mut segment, text_width) {
				break;
			}
		}

		false
	}

	/// Ensures the cursor is visible by adjusting the viewport scroll position.
	///
	/// This function adjusts `self.buffer.scroll_line` and `self.buffer.scroll_segment` to ensure
	/// the primary cursor is visible within the given area. It also updates
	/// `self.buffer.text_width` to match the current rendering context.
	///
	/// # Parameters
	/// - `area`: The rectangular area available for document rendering
	///
	/// # Behavior
	/// - If the cursor is above the viewport, scrolls up to show it
	/// - If the cursor is below the viewport, scrolls down to show it
	/// - Clamps scroll position to valid line/segment boundaries
	pub fn ensure_cursor_visible(&mut self, area: Rect) {
		let total_lines = self.buffer().doc.len_lines();
		let gutter_width = self.gutter_width();
		let text_width = area.width.saturating_sub(gutter_width) as usize;
		let viewport_height = area.height as usize;

		self.buffer_mut().text_width = text_width;

		if self.buffer().scroll_line >= total_lines {
			self.buffer_mut().scroll_line = total_lines.saturating_sub(1);
			self.buffer_mut().scroll_segment = 0;
		}
		let scroll_line = self.buffer().scroll_line;
		let scroll_segment = self.buffer().scroll_segment;
		self.buffer_mut().scroll_segment =
			self.clamp_segment_for_line(scroll_line, scroll_segment, text_width);

		let cursor_pos: CharIdx = self.buffer().cursor;
		let cursor_line = self.cursor_line();
		let cursor_line_start: CharIdx = self.buffer().doc.line_to_char(cursor_line);
		let cursor_col = cursor_pos.saturating_sub(cursor_line_start);

		let cursor_line_end: CharIdx = if cursor_line + 1 < total_lines {
			self.buffer().doc.line_to_char(cursor_line + 1)
		} else {
			self.buffer().doc.len_chars()
		};
		let cursor_line_text: String = self
			.buffer()
			.doc
			.slice(cursor_line_start..cursor_line_end)
			.into();
		let cursor_line_text = cursor_line_text.trim_end_matches('\n');
		let cursor_segments = self.wrap_line(cursor_line_text, text_width);
		let cursor_segment = self.find_segment_for_col(&cursor_segments, cursor_col);

		if cursor_line < self.buffer().scroll_line
			|| (cursor_line == self.buffer().scroll_line
				&& cursor_segment < self.buffer().scroll_segment)
		{
			self.buffer_mut().scroll_line = cursor_line;
			self.buffer_mut().scroll_segment = cursor_segment;
			return;
		}

		let mut prev_scroll = (self.buffer().scroll_line, self.buffer().scroll_segment);
		while !self.cursor_visible_from(
			self.buffer().scroll_line,
			self.buffer().scroll_segment,
			cursor_line,
			cursor_segment,
			viewport_height,
			text_width,
		) {
			self.scroll_viewport_down();

			let new_scroll = (self.buffer().scroll_line, self.buffer().scroll_segment);
			if new_scroll == prev_scroll {
				break;
			}
			prev_scroll = new_scroll;
		}
	}
}
