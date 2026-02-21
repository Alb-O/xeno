//! Cursor navigation for buffers.

use xeno_primitives::{Direction as MoveDir, Range, ScrollDirection, Selection, max_cursor_pos, visible_line_count};

use super::Buffer;
use crate::render::wrap::WrapSegment;

/// Maps a screen column to a character offset within a wrap segment.
///
/// Returns `(offset, matched)` where `matched` is true if `text_col` fell
/// within the segment's content, false if it was past the end.
///
/// # Boundary Rule
/// For non-last segments, positions at the end of the segment content map to
/// the start of the next segment (handled by the caller's visual row logic).
fn col_to_char_offset(segment: &WrapSegment, text_col: usize, tab_width: usize) -> (usize, bool) {
	if segment.text.is_empty() {
		return (segment.start_offset, false);
	}

	let mut col = 0;
	let mut last_i = 0;
	for (i, ch) in segment.text.chars().enumerate() {
		last_i = i;
		let w = if ch == '\t' { tab_width.saturating_sub(col % tab_width).max(1) } else { 1 };
		if text_col < col + w {
			return (segment.start_offset + i, true);
		}
		col += w;
	}
	(segment.start_offset + last_i, false)
}

impl Buffer {
	/// Moves cursors vertically, accounting for line wrapping.
	///
	/// Uses the remembered goal column to restore horizontal position when
	/// crossing short or empty lines. The goal column is set from the primary
	/// cursor's position on the first vertical motion, then preserved until
	/// a horizontal motion resets it.
	///
	/// # Parameters
	/// * `direction`: Forward (down) or Backward (up)
	/// * `count`: Number of visual lines to move
	/// * `extend`: Whether to extend selection
	/// * `tab_width`: Number of spaces a tab character occupies (from options)
	pub fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool, tab_width: usize) {
		self.ensure_valid_selection();
		let ranges = self.selection.ranges().to_vec();
		let primary_index = self.selection.primary_index();

		let goal_col = self.goal_column.unwrap_or_else(|| {
			let primary = &ranges[primary_index];
			self.compute_column_in_line(primary.head)
		});
		self.goal_column = Some(goal_col);

		let mut new_ranges = Vec::with_capacity(ranges.len());

		for range in ranges.iter() {
			let mut pos = range.head;
			for _ in 0..count {
				pos = self.visual_move_from(pos, direction, tab_width, goal_col);
			}

			let new_range = if extend {
				let mut r = *range;
				r.head = pos;
				r
			} else {
				Range::point(pos)
			};

			new_ranges.push(new_range);
		}

		self.selection = Selection::from_vec(new_ranges, primary_index);
		self.cursor = self.selection.primary().head;
	}

	/// Computes the column position of a cursor within its line.
	fn compute_column_in_line(&self, cursor: usize) -> usize {
		self.with_doc(|doc| {
			let line = doc.content().char_to_line(cursor);
			let line_start = doc.content().line_to_char(line);
			cursor.saturating_sub(line_start)
		})
	}

	/// Computes a new cursor position from visual line movement.
	///
	/// Uses `goal_col` to restore horizontal position when the target line
	/// is long enough.
	fn visual_move_from(&self, cursor: usize, direction: MoveDir, tab_width: usize, goal_col: usize) -> usize {
		let (_doc_line, line_start, _total_lines, line_text, next_line_data, prev_line_data) = self.with_doc(|doc| {
			let content = doc.content();
			let doc_line = content.char_to_line(cursor);
			let line_start = content.line_to_char(doc_line);
			let total_lines = visible_line_count(content.slice(..));

			let line_end = if doc_line + 1 < content.len_lines() {
				content.line_to_char(doc_line + 1)
			} else {
				content.len_chars()
			};
			let line_text: String = content.slice(line_start..line_end).into();

			let next_line_data = if doc_line + 1 < total_lines {
				let next_line_start = content.line_to_char(doc_line + 1);
				let next_line_end = if doc_line + 2 < content.len_lines() {
					content.line_to_char(doc_line + 2)
				} else {
					content.len_chars()
				};
				let text: String = content.slice(next_line_start..next_line_end).into();
				Some((next_line_start, text))
			} else {
				None
			};

			let prev_line_data = if doc_line > 0 {
				let prev_line = doc_line - 1;
				let prev_line_start = content.line_to_char(prev_line);
				let text: String = content.slice(prev_line_start..line_start).into();
				Some((prev_line_start, text))
			} else {
				None
			};

			(doc_line, line_start, total_lines, line_text, next_line_data, prev_line_data)
		});

		let line_text = line_text.trim_end_matches('\n');
		let col_in_line = cursor.saturating_sub(line_start);

		let segments = self.wrap_line(line_text, self.text_width, tab_width);
		let current_seg_idx = self.find_segment_for_col(&segments, col_in_line);

		match direction {
			MoveDir::Forward => {
				if current_seg_idx + 1 < segments.len() {
					let next_seg = &segments[current_seg_idx + 1];
					let seg_len = next_seg.text.chars().count().saturating_sub(1);
					let col_in_seg = goal_col.saturating_sub(next_seg.start_offset);
					line_start + next_seg.start_offset + col_in_seg.min(seg_len)
				} else if let Some((next_line_start, next_line_text)) = next_line_data {
					let has_newline = next_line_text.ends_with('\n');
					let next_line_text = next_line_text.trim_end_matches('\n');
					let next_segments = self.wrap_line(next_line_text, self.text_width, tab_width);

					if next_segments.is_empty() {
						next_line_start
					} else {
						let first_seg = &next_segments[0];
						let is_last_seg = next_segments.len() == 1;
						let seg_char_count = first_seg.text.chars().count();
						let seg_len = if is_last_seg && has_newline {
							seg_char_count
						} else {
							seg_char_count.saturating_sub(1)
						};
						next_line_start + goal_col.min(seg_len)
					}
				} else {
					cursor
				}
			}
			MoveDir::Backward => {
				if current_seg_idx > 0 {
					let prev_seg = &segments[current_seg_idx - 1];
					let seg_len = prev_seg.text.chars().count().saturating_sub(1);
					let col_in_seg = goal_col.saturating_sub(prev_seg.start_offset);
					line_start + prev_seg.start_offset + col_in_seg.min(seg_len)
				} else if let Some((prev_line_start, prev_line_text)) = prev_line_data {
					let has_newline = prev_line_text.ends_with('\n');
					let prev_line_text = prev_line_text.trim_end_matches('\n');
					let prev_segments = self.wrap_line(prev_line_text, self.text_width, tab_width);

					if prev_segments.is_empty() {
						prev_line_start
					} else {
						let last_seg = prev_segments.last().unwrap();
						let seg_char_count = last_seg.text.chars().count();
						let seg_len = if has_newline { seg_char_count } else { seg_char_count.saturating_sub(1) };
						prev_line_start + last_seg.start_offset + goal_col.min(seg_len)
					}
				} else {
					cursor
				}
			}
		}
	}

	/// Finds which wrap segment contains the given column.
	pub fn find_segment_for_col(&self, segments: &[WrapSegment], col: usize) -> usize {
		for (i, seg) in segments.iter().enumerate() {
			let seg_end = seg.start_offset + seg.text.chars().count();
			if col < seg_end || i == segments.len() - 1 {
				return i;
			}
		}
		0
	}

	/// Scrolls the viewport without moving the cursor.
	///
	/// Sets [`Self::suppress_auto_scroll`] to prevent the viewport from chasing the
	/// cursor back into view.
	pub fn handle_mouse_scroll(&mut self, direction: ScrollDirection, count: usize, tab_width: usize) {
		match direction {
			ScrollDirection::Up => {
				for _ in 0..count {
					self.scroll_viewport_up(tab_width);
				}
			}
			ScrollDirection::Down => {
				for _ in 0..count {
					self.scroll_viewport_down(tab_width);
				}
			}
			ScrollDirection::Left | ScrollDirection::Right => {}
		}
		self.suppress_auto_scroll = true;
	}

	/// Scrolls viewport up by one visual line.
	pub fn scroll_viewport_up(&mut self, tab_width: usize) {
		if self.scroll_segment > 0 {
			self.scroll_segment -= 1;
		} else if self.scroll_line > 0 {
			self.scroll_line -= 1;
			let num_segments = self.with_doc(|doc| {
				let line_slice = doc.content().line(self.scroll_line);
				let line_len = line_slice.len_chars();
				let has_newline = line_len > 0 && line_slice.char(line_len - 1) == '\n';
				let content = if has_newline { line_slice.slice(..line_len - 1) } else { line_slice };
				let text: String = content.into();
				let segments = self.wrap_line(&text, self.text_width, tab_width);
				segments.len().max(1)
			});
			self.scroll_segment = num_segments.saturating_sub(1);
		}
	}

	/// Scrolls viewport down by one visual line.
	pub fn scroll_viewport_down(&mut self, tab_width: usize) {
		let (total_lines, num_segments) = self.with_doc(|doc| {
			let content = doc.content();
			let total_lines = visible_line_count(content.slice(..));
			if self.scroll_line < total_lines {
				let line_slice = content.line(self.scroll_line);
				let line_len = line_slice.len_chars();
				let has_newline = line_len > 0 && line_slice.char(line_len - 1) == '\n';
				let content = if has_newline { line_slice.slice(..line_len - 1) } else { line_slice };
				let text: String = content.into();
				let segments = self.wrap_line(&text, self.text_width, tab_width);
				(total_lines, segments.len().max(1))
			} else {
				(total_lines, 1)
			}
		});

		if self.scroll_line < total_lines {
			if self.scroll_segment + 1 < num_segments {
				self.scroll_segment += 1;
			} else if self.scroll_line + 1 < total_lines {
				self.scroll_line += 1;
				self.scroll_segment = 0;
			}
		}
	}

	/// Converts screen coordinates to document position.
	///
	/// Returns `None` for clicks in the gutter area within document bounds.
	/// Clicks below the document map to the corresponding column on the last line.
	pub fn screen_to_doc_position(&self, screen_row: u16, screen_col: u16, tab_width: usize) -> Option<usize> {
		let gutter_width = self.gutter_width();
		let in_gutter = screen_col < gutter_width;
		let text_col = screen_col.saturating_sub(gutter_width) as usize;
		let mut visual_row = 0;
		let mut line_idx = self.scroll_line;
		let mut start_segment = self.scroll_segment;

		self.with_doc(|doc| {
			let content = doc.content();
			let total_lines = visible_line_count(content.slice(..));
			let max_pos = max_cursor_pos(content.slice(..));

			while line_idx < total_lines {
				let line_start = content.line_to_char(line_idx);
				let line_text = content.line(line_idx);
				let line_len = line_text.len_chars();
				let has_newline = line_len > 0 && line_text.char(line_len - 1) == '\n';

				let text_for_wrap: String = if has_newline {
					line_text.slice(..line_len - 1).into()
				} else {
					line_text.into()
				};
				let segments = self.wrap_line(&text_for_wrap, self.text_width, tab_width);

				if segments.is_empty() {
					if visual_row == screen_row as usize {
						return if in_gutter { None } else { Some(line_start) };
					}
					visual_row += 1;
				} else {
					let num_segments = segments.len();
					for (seg_idx, segment) in segments.iter().enumerate().skip(start_segment) {
						if visual_row == screen_row as usize {
							if in_gutter {
								return None;
							}

							let (offset, matched) = col_to_char_offset(segment, text_col, tab_width);
							let is_last_seg = seg_idx == num_segments - 1;

							if !matched && !is_last_seg {
								let next_segment = &segments[seg_idx + 1];
								return Some((line_start + next_segment.start_offset).min(max_pos));
							} else {
								let past_end_offset = if !matched { if has_newline { 1 } else { 0 } } else { 0 };
								return Some((line_start + offset + past_end_offset).min(max_pos));
							}
						}
						visual_row += 1;
					}
				}

				start_segment = 0;
				line_idx += 1;
			}

			Some(max_pos)
		})
	}

	/// Converts a document position to screen coordinates within the buffer view.
	///
	/// Returns None if the position is above the current scroll window.
	pub fn doc_to_screen_position(&self, doc_pos: usize, tab_width: usize) -> Option<(u16, u16)> {
		self.with_doc(|doc| {
			let content = doc.content();
			let total_lines = visible_line_count(content.slice(..));
			let line_idx = content.char_to_line(doc_pos.min(content.len_chars()));
			if line_idx < self.scroll_line || self.scroll_line >= total_lines {
				return None;
			}

			let line_start = content.line_to_char(line_idx);
			let col_in_line = doc_pos.saturating_sub(line_start);
			let gutter_width = self.gutter_width_with_doc(doc) as usize;

			let mut visual_row = 0usize;
			let mut current_line = self.scroll_line;
			let mut start_segment = self.scroll_segment;

			while current_line <= line_idx {
				let line_start = content.line_to_char(current_line);
				let line_end = if current_line + 1 < content.len_lines() {
					content.line_to_char(current_line + 1)
				} else {
					content.len_chars()
				};

				let line_text: String = content.slice(line_start..line_end).into();
				let line_text = line_text.trim_end_matches('\n');
				let segments = self.wrap_line(line_text, self.text_width, tab_width);

				if current_line == line_idx {
					if segments.is_empty() {
						let row = visual_row as u16;
						let col = gutter_width as u16;
						return Some((row, col));
					}

					let mut seg_row = visual_row;
					let num_segments = segments.len();
					for (seg_idx, segment) in segments.iter().enumerate().skip(start_segment) {
						let seg_start = segment.start_offset;
						let seg_len = segment.text.chars().count();
						let seg_end = seg_start + seg_len;

						let is_last_seg = seg_idx == num_segments - 1;
						if col_in_line < seg_end || is_last_seg {
							let offset = col_in_line.saturating_sub(seg_start);
							let mut col = 0usize;
							for (idx, ch) in segment.text.chars().enumerate() {
								if idx >= offset {
									break;
								}
								let mut w = if ch == '\t' { tab_width.saturating_sub(col % tab_width) } else { 1 };
								if w == 0 {
									w = 1;
								}
								let remaining = self.text_width.saturating_sub(col);
								if remaining == 0 {
									break;
								}
								if w > remaining {
									w = remaining;
								}
								col += w;
							}

							let row = seg_row as u16;
							let col = gutter_width.saturating_add(col) as u16;
							return Some((row, col));
						}
						seg_row += 1;
					}

					let row = visual_row.saturating_add(segments.len().saturating_sub(start_segment).saturating_sub(1)) as u16;
					let col = gutter_width as u16;
					return Some((row, col));
				}

				let visible_segments = if segments.is_empty() {
					1
				} else {
					segments.len().saturating_sub(start_segment)
				};
				visual_row = visual_row.saturating_add(visible_segments);
				start_segment = 0;
				current_line += 1;
			}

			None
		})
	}

	/// Wraps a line of text into segments.
	///
	/// # Parameters
	/// * `text`: The text to wrap
	/// * `width`: Maximum width in characters for each segment
	/// * `tab_width`: Number of spaces a tab character occupies
	pub fn wrap_line(&self, text: &str, width: usize, tab_width: usize) -> Vec<WrapSegment> {
		crate::render::wrap_line(text, width, tab_width)
	}
}

#[cfg(test)]
mod tests;
