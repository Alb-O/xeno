use crate::layout::{Position, Rect};

/// An iterator over rows within a `Rect`.
pub struct Rows {
	/// The `Rect` associated with the rows.
	rect: Rect,
	/// The y coordinate of the row within the `Rect` when iterating forwards.
	current_row_fwd: u16,
	/// The y coordinate of the row within the `Rect` when iterating backwards.
	current_row_back: u16,
}

impl Rows {
	/// Creates a new `Rows` iterator.
	pub const fn new(rect: Rect) -> Self {
		Self {
			rect,
			current_row_fwd: rect.y,
			current_row_back: rect.bottom(),
		}
	}
}

impl Iterator for Rows {
	type Item = Rect;

	/// Retrieves the next row within the `Rect`.
	///
	/// Returns `None` when there are no more rows to iterate through.
	fn next(&mut self) -> Option<Self::Item> {
		if self.current_row_fwd >= self.current_row_back {
			return None;
		}
		let row = Rect::new(self.rect.x, self.current_row_fwd, self.rect.width, 1);
		self.current_row_fwd += 1;
		Some(row)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let start_count = self.current_row_fwd.saturating_sub(self.rect.top());
		let end_count = self.rect.bottom().saturating_sub(self.current_row_back);
		let count = self.rect.height.saturating_sub(start_count).saturating_sub(end_count) as usize;
		(count, Some(count))
	}
}

impl DoubleEndedIterator for Rows {
	/// Retrieves the previous row within the `Rect`.
	///
	/// Returns `None` when there are no more rows to iterate through.
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.current_row_back <= self.current_row_fwd {
			return None;
		}
		self.current_row_back -= 1;
		let row = Rect::new(self.rect.x, self.current_row_back, self.rect.width, 1);
		Some(row)
	}
}

/// An iterator over columns within a `Rect`.
pub struct Columns {
	/// The `Rect` associated with the columns.
	rect: Rect,
	/// The x coordinate of the column within the `Rect` when iterating forwards.
	current_column_fwd: u16,
	/// The x coordinate of the column within the `Rect` when iterating backwards.
	current_column_back: u16,
}

impl Columns {
	/// Creates a new `Columns` iterator.
	pub const fn new(rect: Rect) -> Self {
		Self {
			rect,
			current_column_fwd: rect.x,
			current_column_back: rect.right(),
		}
	}
}

impl Iterator for Columns {
	type Item = Rect;

	/// Retrieves the next column within the `Rect`.
	///
	/// Returns `None` when there are no more columns to iterate through.
	fn next(&mut self) -> Option<Self::Item> {
		if self.current_column_fwd >= self.current_column_back {
			return None;
		}
		let column = Rect::new(self.current_column_fwd, self.rect.y, 1, self.rect.height);
		self.current_column_fwd += 1;
		Some(column)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let start_count = self.current_column_fwd.saturating_sub(self.rect.left());
		let end_count = self.rect.right().saturating_sub(self.current_column_back);
		let count = self.rect.width.saturating_sub(start_count).saturating_sub(end_count) as usize;
		(count, Some(count))
	}
}

impl DoubleEndedIterator for Columns {
	/// Retrieves the previous column within the `Rect`.
	///
	/// Returns `None` when there are no more columns to iterate through.
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.current_column_back <= self.current_column_fwd {
			return None;
		}
		self.current_column_back -= 1;
		let column = Rect::new(self.current_column_back, self.rect.y, 1, self.rect.height);
		Some(column)
	}
}

/// An iterator over positions within a `Rect`.
///
/// The iterator will yield all positions within the `Rect` in a row-major order.
pub struct Positions {
	/// The `Rect` associated with the positions.
	rect: Rect,
	/// The current position within the `Rect`.
	current_position: Position,
}

impl Positions {
	/// Creates a new `Positions` iterator.
	pub const fn new(rect: Rect) -> Self {
		Self {
			rect,
			current_position: Position::new(rect.x, rect.y),
		}
	}
}

impl Iterator for Positions {
	type Item = Position;

	/// Retrieves the next position within the `Rect`.
	///
	/// Returns `None` when there are no more positions to iterate through.
	fn next(&mut self) -> Option<Self::Item> {
		if !self.rect.contains(self.current_position) {
			return None;
		}
		let position = self.current_position;
		self.current_position.x += 1;
		if self.current_position.x >= self.rect.right() {
			self.current_position.x = self.rect.x;
			self.current_position.y += 1;
		}
		Some(position)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let row_count = self.rect.bottom().saturating_sub(self.current_position.y);
		if row_count == 0 {
			return (0, Some(0));
		}
		let column_count = self.rect.right().saturating_sub(self.current_position.x);
		// subtract 1 from the row count to account for the current row
		let count = (row_count - 1).saturating_mul(self.rect.width).saturating_add(column_count) as usize;
		(count, Some(count))
	}
}

#[cfg(test)]
mod tests;
