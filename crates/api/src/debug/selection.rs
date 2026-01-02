//! Debug panel text selection.

/// Selection state for debug panel text.
#[derive(Debug, Clone, Copy)]
pub struct DebugSelection {
	/// Row where the selection started.
	pub anchor_row: u16,
	/// Column where the selection started.
	pub anchor_col: u16,
	/// Current cursor row position.
	pub cursor_row: u16,
	/// Current cursor column position.
	pub cursor_col: u16,
}

impl DebugSelection {
	/// Returns (start_row, start_col, end_row, end_col) in normalized order.
	pub fn bounds(&self) -> (u16, u16, u16, u16) {
		if (self.anchor_row, self.anchor_col) <= (self.cursor_row, self.cursor_col) {
			(
				self.anchor_row,
				self.anchor_col,
				self.cursor_row,
				self.cursor_col,
			)
		} else {
			(
				self.cursor_row,
				self.cursor_col,
				self.anchor_row,
				self.anchor_col,
			)
		}
	}

	/// Returns true if the given cell is within the selection.
	pub fn contains(&self, row: u16, col: u16) -> bool {
		let (start_row, start_col, end_row, end_col) = self.bounds();
		if row < start_row || row > end_row {
			return false;
		}
		if row == start_row && row == end_row {
			col >= start_col && col <= end_col
		} else if row == start_row {
			col >= start_col
		} else if row == end_row {
			col <= end_col
		} else {
			true
		}
	}
}
