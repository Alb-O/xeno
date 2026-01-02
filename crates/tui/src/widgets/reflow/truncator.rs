//! Line truncating line composer.
use alloc::vec;
use alloc::vec::Vec;

use unicode_width::UnicodeWidthStr;

use super::{trim_offset, LineComposer, WrappedLine};
use crate::layout::HorizontalAlignment;
use crate::text::StyledGrapheme;

/// A state machine that truncates overhanging lines.
#[derive(Debug, Default, Clone)]
pub struct LineTruncator<'a, O, I>
where
	O: Iterator<Item = (I, HorizontalAlignment)>,
	I: Iterator<Item = StyledGrapheme<'a>>,
{
	/// The given, unprocessed lines
	input_lines: O,
	max_line_width: u16,
	current_line: Vec<StyledGrapheme<'a>>,
	/// Record the offset to skip render
	horizontal_offset: u16,
}

impl<'a, O, I> LineTruncator<'a, O, I>
where
	O: Iterator<Item = (I, HorizontalAlignment)>,
	I: Iterator<Item = StyledGrapheme<'a>>,
{
	/// Create a new `LineTruncator` with the given lines and maximum line width.
	pub const fn new(lines: O, max_line_width: u16) -> Self {
		Self {
			input_lines: lines,
			max_line_width,
			horizontal_offset: 0,
			current_line: vec![],
		}
	}

	/// Set the horizontal offset to skip render.
	pub const fn set_horizontal_offset(&mut self, horizontal_offset: u16) {
		self.horizontal_offset = horizontal_offset;
	}
}

impl<'a, O, I> LineComposer<'a> for LineTruncator<'a, O, I>
where
	O: Iterator<Item = (I, HorizontalAlignment)>,
	I: Iterator<Item = StyledGrapheme<'a>>,
{
	fn next_line<'lend>(&'lend mut self) -> Option<WrappedLine<'lend, 'a>> {
		if self.max_line_width == 0 {
			return None;
		}

		self.current_line.truncate(0);
		let mut current_line_width = 0;

		let mut lines_exhausted = true;
		let mut horizontal_offset = self.horizontal_offset as usize;
		let mut current_alignment = HorizontalAlignment::Left;
		if let Some((current_line, alignment)) = &mut self.input_lines.next() {
			lines_exhausted = false;
			current_alignment = *alignment;

			for StyledGrapheme { symbol, style } in current_line {
				// Ignore characters wider that the total max width.
				if symbol.width() as u16 > self.max_line_width {
					continue;
				}

				if current_line_width + symbol.width() as u16 > self.max_line_width {
					break;
				}

				let symbol = if horizontal_offset == 0 || HorizontalAlignment::Left != *alignment {
					symbol
				} else {
					let w = symbol.width();
					if w > horizontal_offset {
						let t = trim_offset(symbol, horizontal_offset);
						horizontal_offset = 0;
						t
					} else {
						horizontal_offset -= w;
						""
					}
				};
				current_line_width += symbol.width() as u16;
				self.current_line.push(StyledGrapheme { symbol, style });
			}
		}

		if lines_exhausted {
			None
		} else {
			Some(WrappedLine {
				graphemes: &self.current_line,
				width: current_line_width,
				alignment: current_alignment,
			})
		}
	}
}
