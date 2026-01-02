//! Internal module for reflowing text to fit into a certain width.
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::layout::HorizontalAlignment;
use crate::text::StyledGrapheme;

mod truncator;
mod wrapper;

pub use truncator::LineTruncator;
pub use wrapper::WordWrapper;

#[cfg(test)]
mod tests;

/// A state machine to pack styled symbols into lines.
/// Cannot implement it as Iterator since it yields slices of the internal buffer (need streaming
/// iterators for that).
pub trait LineComposer<'a> {
	/// Returns the next wrapped line, or None if exhausted.
	fn next_line<'lend>(&'lend mut self) -> Option<WrappedLine<'lend, 'a>>;
}

/// A line that has been wrapped to a certain width.
pub struct WrappedLine<'lend, 'text> {
	/// One line reflowed to the correct width
	pub graphemes: &'lend [StyledGrapheme<'text>],
	/// The width of the line
	pub width: u16,
	/// Whether the line was aligned left or right
	pub alignment: HorizontalAlignment,
}

/// This function will return a str slice which start at specified offset.
/// As src is a unicode str, start offset has to be calculated with each character.
pub(crate) fn trim_offset(src: &str, mut offset: usize) -> &str {
	let mut start = 0;
	for c in UnicodeSegmentation::graphemes(src, true) {
		let w = c.width();
		if w <= offset {
			offset -= w;
			start += c.len();
		} else {
			break;
		}
	}
	#[expect(clippy::string_slice)] // Is safe as it comes from UnicodeSegmentation
	&src[start..]
}
