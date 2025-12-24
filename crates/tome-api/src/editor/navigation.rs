use tome_core::range::{Direction as MoveDir, Range};
use tome_core::{ScrollDirection, Selection};

use super::Editor;
use crate::render::WrapSegment;

impl Editor {
	pub fn cursor_line(&self) -> usize {
		let max_pos = self.doc.len_chars();
		self.doc.char_to_line(self.cursor.min(max_pos))
	}

	pub fn cursor_col(&self) -> usize {
		let line = self.cursor_line();
		let line_start = self.doc.line_to_char(line);
		self.cursor.saturating_sub(line_start)
	}

	/// Minimum gutter width padding (extra digits reserved beyond current line count).
	const GUTTER_MIN_WIDTH: u16 = 4;

	/// Compute the gutter width based on total line count.
	pub fn gutter_width(&self) -> u16 {
		let total_lines = self.doc.len_lines();
		(total_lines.max(1).ilog10() as u16 + 2).max(Self::GUTTER_MIN_WIDTH)
	}

	pub fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool) {
		let ranges = self.selection.ranges().to_vec();
		let primary_index = self.selection.primary_index();
		let mut new_ranges = Vec::with_capacity(ranges.len());

		for (idx, range) in ranges.iter().enumerate() {
			let mut pos = range.head;
			for _ in 0..count {
				pos = self.visual_move_from(pos, direction);
			}

			let mut new_range = if extend { *range } else { Range::point(pos) };
			if extend {
				new_range.head = pos;
			}

			if idx == primary_index {
				// We'll reset cursor after rebuilding selection; keep tracking via index.
			}
			new_ranges.push(new_range);
		}

		self.selection = Selection::from_vec(new_ranges, primary_index);
		self.cursor = self.selection.primary().head;
	}

	fn visual_move_from(&self, cursor: usize, direction: MoveDir) -> usize {
		let doc_line = self.doc.char_to_line(cursor);
		let line_start = self.doc.line_to_char(doc_line);
		let col_in_line = cursor.saturating_sub(line_start);

		let total_lines = self.doc.len_lines();
		let _line_end = if doc_line + 1 < total_lines {
			self.doc.line_to_char(doc_line + 1)
		} else {
			self.doc.len_chars()
		};
		let line_text: String = self.doc.slice(line_start.._line_end).into();
		let line_text = line_text.trim_end_matches('\n');

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
				} else if doc_line + 1 < total_lines {
					let next_line_start = self.doc.line_to_char(doc_line + 1);
					let next_line_end = if doc_line + 2 < total_lines {
						self.doc.line_to_char(doc_line + 2)
					} else {
						self.doc.len_chars()
					};
					let next_line_text: String =
						self.doc.slice(next_line_start..next_line_end).into();
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
				} else if doc_line > 0 {
					let prev_line = doc_line - 1;
					let prev_line_start = self.doc.line_to_char(prev_line);
					let prev_line_end = line_start;
					let prev_line_text: String =
						self.doc.slice(prev_line_start..prev_line_end).into();
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

	pub fn find_segment_for_col(&self, segments: &[WrapSegment], col: usize) -> usize {
		for (i, seg) in segments.iter().enumerate() {
			let seg_end = seg.start_offset + seg.text.chars().count();
			if col < seg_end || i == segments.len() - 1 {
				return i;
			}
		}
		0
	}

	pub(crate) fn handle_mouse_scroll(&mut self, direction: ScrollDirection, count: usize) {
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

	pub(crate) fn scroll_viewport_up(&mut self) {
		if self.scroll_segment > 0 {
			self.scroll_segment -= 1;
		} else if self.scroll_line > 0 {
			self.scroll_line -= 1;
			let line_start = self.doc.line_to_char(self.scroll_line);
			let line_end = if self.scroll_line + 1 < self.doc.len_lines() {
				self.doc.line_to_char(self.scroll_line + 1)
			} else {
				self.doc.len_chars()
			};
			let line_text: String = self.doc.slice(line_start..line_end).into();
			let line_text = line_text.trim_end_matches('\n');
			let segments = self.wrap_line(line_text, self.text_width);
			self.scroll_segment = segments.len().saturating_sub(1);
		}
	}

	pub(crate) fn scroll_viewport_down(&mut self) {
		let total_lines = self.doc.len_lines();
		if self.scroll_line < total_lines {
			let line_start = self.doc.line_to_char(self.scroll_line);
			let line_end = if self.scroll_line + 1 < total_lines {
				self.doc.line_to_char(self.scroll_line + 1)
			} else {
				self.doc.len_chars()
			};
			let line_text: String = self.doc.slice(line_start..line_end).into();
			let line_text = line_text.trim_end_matches('\n');
			let segments = self.wrap_line(line_text, self.text_width);
			let num_segments = segments.len().max(1);

			if self.scroll_segment + 1 < num_segments {
				self.scroll_segment += 1;
			} else if self.scroll_line + 1 < total_lines {
				self.scroll_line += 1;
				self.scroll_segment = 0;
			}
		}
	}

	pub(crate) fn screen_to_doc_position(&self, screen_row: u16, screen_col: u16) -> Option<usize> {
		let total_lines = self.doc.len_lines();
		let gutter_width = self.gutter_width();

		if screen_col < gutter_width {
			return None;
		}

		let text_col = (screen_col - gutter_width) as usize;
		let mut visual_row = 0;
		let mut line_idx = self.scroll_line;
		let mut start_segment = self.scroll_segment;

		while line_idx < total_lines {
			let line_start = self.doc.line_to_char(line_idx);
			let line_end = if line_idx + 1 < total_lines {
				self.doc.line_to_char(line_idx + 1)
			} else {
				self.doc.len_chars()
			};

			let line_text: String = self.doc.slice(line_start..line_end).into();
			let line_text = line_text.trim_end_matches('\n');
			let segments = self.wrap_line(line_text, self.text_width);

			if segments.is_empty() {
				if visual_row == screen_row as usize {
					return Some(line_start);
				}
				visual_row += 1;
			} else {
				let tab_width = 4usize;
				for (_seg_idx, segment) in segments.iter().enumerate().skip(start_segment) {
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

		Some(self.doc.len_chars().saturating_sub(1).max(0))
	}
}
