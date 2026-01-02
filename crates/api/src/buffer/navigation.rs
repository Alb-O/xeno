//! Cursor navigation for buffers.

use evildoer_base::range::{Direction as MoveDir, Range};
use evildoer_base::{ScrollDirection, Selection};

use super::Buffer;
use crate::render::WrapSegment;

impl Buffer {
	/// Moves cursors vertically, accounting for line wrapping.
	pub fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool) {
		self.ensure_valid_selection();
		let ranges = self.selection.ranges().to_vec();
		let primary_index = self.selection.primary_index();
		let mut new_ranges = Vec::with_capacity(ranges.len());

		for range in ranges.iter() {
			let mut pos = range.head;
			for _ in 0..count {
				pos = self.visual_move_from(pos, direction);
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

	/// Computes a new cursor position from visual line movement.
	fn visual_move_from(&self, cursor: usize, direction: MoveDir) -> usize {
		// Extract all needed data from doc in one block
		let (_doc_line, line_start, _total_lines, line_text, next_line_data, prev_line_data) = {
			let doc = self.doc();
			let doc_line = doc.content.char_to_line(cursor);
			let line_start = doc.content.line_to_char(doc_line);
			let total_lines = doc.content.len_lines();

			let line_end = if doc_line + 1 < total_lines {
				doc.content.line_to_char(doc_line + 1)
			} else {
				doc.content.len_chars()
			};
			let line_text: String = doc.content.slice(line_start..line_end).into();

			// Get next line data if needed
			let next_line_data = if doc_line + 1 < total_lines {
				let next_line_start = doc.content.line_to_char(doc_line + 1);
				let next_line_end = if doc_line + 2 < total_lines {
					doc.content.line_to_char(doc_line + 2)
				} else {
					doc.content.len_chars()
				};
				let text: String = doc.content.slice(next_line_start..next_line_end).into();
				Some((next_line_start, text))
			} else {
				None
			};

			// Get prev line data if needed
			let prev_line_data = if doc_line > 0 {
				let prev_line = doc_line - 1;
				let prev_line_start = doc.content.line_to_char(prev_line);
				let text: String = doc.content.slice(prev_line_start..line_start).into();
				Some((prev_line_start, text))
			} else {
				None
			};

			(
				doc_line,
				line_start,
				total_lines,
				line_text,
				next_line_data,
				prev_line_data,
			)
		};

		let line_text = line_text.trim_end_matches('\n');
		let col_in_line = cursor.saturating_sub(line_start);

		let segments = self.wrap_line(line_text, self.text_width);
		let current_seg_idx = self.find_segment_for_col(&segments, col_in_line);
		let col_in_seg = if current_seg_idx < segments.len() {
			col_in_line.saturating_sub(segments[current_seg_idx].start_offset)
		} else {
			col_in_line
		};

		match direction {
			MoveDir::Forward => {
				if current_seg_idx + 1 < segments.len() {
					let next_seg = &segments[current_seg_idx + 1];
					let new_col = next_seg.start_offset
						+ col_in_seg.min(next_seg.text.chars().count().saturating_sub(1));
					line_start + new_col
				} else if let Some((next_line_start, next_line_text)) = next_line_data {
					let next_line_text = next_line_text.trim_end_matches('\n');
					let next_segments = self.wrap_line(next_line_text, self.text_width);

					if next_segments.is_empty() {
						next_line_start
					} else {
						let first_seg = &next_segments[0];
						let new_col =
							col_in_seg.min(first_seg.text.chars().count().saturating_sub(1).max(0));
						next_line_start + new_col
					}
				} else {
					cursor
				}
			}
			MoveDir::Backward => {
				if current_seg_idx > 0 {
					let prev_seg = &segments[current_seg_idx - 1];
					let new_col = prev_seg.start_offset
						+ col_in_seg.min(prev_seg.text.chars().count().saturating_sub(1));
					line_start + new_col
				} else if let Some((prev_line_start, prev_line_text)) = prev_line_data {
					let prev_line_text = prev_line_text.trim_end_matches('\n');
					let prev_segments = self.wrap_line(prev_line_text, self.text_width);

					if prev_segments.is_empty() {
						prev_line_start
					} else {
						let last_seg = &prev_segments[prev_segments.len() - 1];
						let new_col = last_seg.start_offset
							+ col_in_seg
								.min(last_seg.text.chars().count().saturating_sub(1).max(0));
						prev_line_start + new_col
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

	/// Handles mouse scroll events.
	pub fn handle_mouse_scroll(&mut self, direction: ScrollDirection, count: usize) {
		self.ensure_valid_selection();
		match direction {
			ScrollDirection::Up => {
				for _ in 0..count {
					self.scroll_viewport_up();
				}
				self.move_visual_vertical(MoveDir::Backward, count, false);
			}
			ScrollDirection::Down => {
				for _ in 0..count {
					self.scroll_viewport_down();
				}
				self.move_visual_vertical(MoveDir::Forward, count, false);
			}
			ScrollDirection::Left | ScrollDirection::Right => {
				// Horizontal scroll not implemented yet
			}
		}
	}

	/// Scrolls viewport up by one visual line.
	pub fn scroll_viewport_up(&mut self) {
		if self.scroll_segment > 0 {
			self.scroll_segment -= 1;
		} else if self.scroll_line > 0 {
			self.scroll_line -= 1;
			let (line_text, num_segments) = {
				let doc = self.doc();
				let line_start = doc.content.line_to_char(self.scroll_line);
				let line_end = if self.scroll_line + 1 < doc.content.len_lines() {
					doc.content.line_to_char(self.scroll_line + 1)
				} else {
					doc.content.len_chars()
				};
				let text: String = doc.content.slice(line_start..line_end).into();
				let segments = self.wrap_line(text.trim_end_matches('\n'), self.text_width);
				(text, segments.len())
			};
			let _ = line_text;
			self.scroll_segment = num_segments.saturating_sub(1);
		}
	}

	/// Scrolls viewport down by one visual line.
	pub fn scroll_viewport_down(&mut self) {
		let (total_lines, num_segments) = {
			let doc = self.doc();
			let total_lines = doc.content.len_lines();
			if self.scroll_line < total_lines {
				let line_start = doc.content.line_to_char(self.scroll_line);
				let line_end = if self.scroll_line + 1 < total_lines {
					doc.content.line_to_char(self.scroll_line + 1)
				} else {
					doc.content.len_chars()
				};
				let line_text: String = doc.content.slice(line_start..line_end).into();
				let segments = self.wrap_line(line_text.trim_end_matches('\n'), self.text_width);
				(total_lines, segments.len().max(1))
			} else {
				(total_lines, 1)
			}
		};

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
	pub fn screen_to_doc_position(&self, screen_row: u16, screen_col: u16) -> Option<usize> {
		let gutter_width = self.gutter_width();

		if screen_col < gutter_width {
			return None;
		}

		let text_col = (screen_col - gutter_width) as usize;
		let mut visual_row = 0;
		let mut line_idx = self.scroll_line;
		let mut start_segment = self.scroll_segment;

		let doc = self.doc();
		let total_lines = doc.content.len_lines();

		while line_idx < total_lines {
			let line_start = doc.content.line_to_char(line_idx);
			let line_end = if line_idx + 1 < total_lines {
				doc.content.line_to_char(line_idx + 1)
			} else {
				doc.content.len_chars()
			};

			let line_text: String = doc.content.slice(line_start..line_end).into();
			let line_text = line_text.trim_end_matches('\n');
			let segments = self.wrap_line(line_text, self.text_width);

			if segments.is_empty() {
				if visual_row == screen_row as usize {
					return Some(line_start);
				}
				visual_row += 1;
			} else {
				let tab_width = 4usize;
				for segment in segments.iter().skip(start_segment) {
					if visual_row == screen_row as usize {
						if segment.text.is_empty() {
							return Some(line_start + segment.start_offset);
						}

						let mut col = 0usize;
						let mut last_char_offset = 0usize;
						for (i, ch) in segment.text.chars().enumerate() {
							last_char_offset = i;
							let mut w = if ch == '\t' {
								tab_width.saturating_sub(col % tab_width)
							} else {
								1
							};
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

							if text_col < col + w {
								return Some(line_start + segment.start_offset + i);
							}
							col += w;
						}
						return Some(line_start + segment.start_offset + last_char_offset);
					}
					visual_row += 1;
				}
			}

			start_segment = 0;
			line_idx += 1;
		}

		Some(doc.content.len_chars().saturating_sub(1).max(0))
	}

	/// Wraps a line of text into segments.
	pub fn wrap_line(&self, text: &str, width: usize) -> Vec<WrapSegment> {
		crate::render::wrap_line(text, width)
	}
}
