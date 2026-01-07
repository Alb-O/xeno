//! Editor-level navigation operations.
//!
//! Most navigation is delegated to Buffer. This module provides
//! Editor-specific wrappers where needed.

use xeno_base::ScrollDirection;
use xeno_base::range::Direction as MoveDir;
use xeno_registry::options::keys;

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
	/// Resolves the `tab-width` option and delegates to Buffer.
	pub fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool) {
		let tab_width = self.tab_width();
		self.buffer_mut()
			.move_visual_vertical(direction, count, extend, tab_width);
	}

	/// Handles mouse scroll events.
	///
	/// Resolves `scroll-lines` and `tab-width` options and delegates to Buffer.
	pub(crate) fn handle_mouse_scroll(&mut self, direction: ScrollDirection, count: usize) {
		let scroll_lines = (self.option(keys::SCROLL_LINES) as usize).max(1);
		let tab_width = self.tab_width();
		self.buffer_mut()
			.handle_mouse_scroll(direction, count * scroll_lines, tab_width);
	}
}
