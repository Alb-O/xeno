//! Editor-level navigation operations.
//!
//! Most navigation is delegated to Buffer. This module provides
//! Editor-specific wrappers where needed.

use xeno_base::ScrollDirection;
use xeno_base::range::Direction as MoveDir;

use super::Editor;

impl Editor {
	/// Returns the line number containing the cursor.
	pub fn cursor_line(&self) -> usize {
		self.buffer().cursor_line()
	}

	/// Returns the column of the cursor within its line.
	pub fn cursor_col(&self) -> usize {
		self.buffer().cursor_col()
	}

	/// Computes the gutter width based on total line count.
	pub fn gutter_width(&self) -> u16 {
		self.buffer().gutter_width()
	}

	/// Moves cursors vertically, accounting for line wrapping.
	///
	/// Delegates to Buffer.
	pub fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool) {
		self.buffer_mut()
			.move_visual_vertical(direction, count, extend);
	}

	/// Handles mouse scroll events.
	///
	/// Delegates to Buffer.
	pub(crate) fn handle_mouse_scroll(&mut self, direction: ScrollDirection, count: usize) {
		self.buffer_mut().handle_mouse_scroll(direction, count);
	}
}
