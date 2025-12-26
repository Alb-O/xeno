//! Editor-level navigation operations.
//!
//! Most navigation is delegated to Buffer. This module provides
//! Editor-specific wrappers where needed.

use tome_base::ScrollDirection;
use tome_base::range::Direction as MoveDir;

use super::Editor;
use crate::render::WrapSegment;

impl Editor {
	/// Returns the line number containing the cursor.
	///
	/// Returns 0 if a terminal is focused.
	pub fn cursor_line(&self) -> usize {
		if self.is_terminal_focused() {
			0
		} else {
			self.buffer().cursor_line()
		}
	}

	/// Returns the column of the cursor within its line.
	///
	/// Returns 0 if a terminal is focused.
	pub fn cursor_col(&self) -> usize {
		if self.is_terminal_focused() {
			0
		} else {
			self.buffer().cursor_col()
		}
	}

	/// Computes the gutter width based on total line count.
	///
	/// Returns 0 if a terminal is focused.
	pub fn gutter_width(&self) -> u16 {
		if self.is_terminal_focused() {
			0
		} else {
			self.buffer().gutter_width()
		}
	}

	/// Moves cursors vertically, accounting for line wrapping.
	///
	/// Delegates to Buffer.
	pub fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool) {
		self.buffer_mut()
			.move_visual_vertical(direction, count, extend);
	}

	/// Finds which wrap segment contains the given column.
	///
	/// Delegates to Buffer.
	pub fn find_segment_for_col(&self, segments: &[WrapSegment], col: usize) -> usize {
		self.buffer().find_segment_for_col(segments, col)
	}

	/// Handles mouse scroll events.
	///
	/// Delegates to Buffer.
	pub(crate) fn handle_mouse_scroll(&mut self, direction: ScrollDirection, count: usize) {
		self.buffer_mut().handle_mouse_scroll(direction, count);
	}

	/// Converts screen coordinates to document position.
	///
	/// Delegates to Buffer.
	#[allow(dead_code, reason = "Will be used for mouse click handling")]
	pub(crate) fn screen_to_doc_position(&self, screen_row: u16, screen_col: u16) -> Option<usize> {
		self.buffer().screen_to_doc_position(screen_row, screen_col)
	}
}
