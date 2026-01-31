use std::rc::Rc;

use super::{Constraint, Direction, Rect};

/// Layout engine for splitting terminal space using constraints.
///
/// Divides rectangular areas using constraints (length, percentage, min) and direction
/// (horizontal/vertical). Uses a deterministic three-pass algorithm:
///
/// 1. `Length` constraints are allocated their exact value (clamped to remaining space).
/// 2. `Percentage` constraints receive their proportional share (clamped to remaining).
/// 3. `Min` constraints receive at least their minimum, then split any leftover evenly.
///
/// # Example
///
/// ```rust
/// use xeno_tui::layout::{Constraint, Layout, Rect};
///
/// let layout = Layout::vertical([Constraint::Length(5), Constraint::Min(1)]);
/// let [top, bottom] = layout.areas(Rect::new(0, 0, 80, 24));
/// ```
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Layout {
	/// Layout direction (horizontal or vertical).
	direction: Direction,
	/// Size constraints for each segment.
	constraints: Vec<Constraint>,
}

impl Layout {
	/// Creates a new layout with the given direction and constraints.
	///
	/// Constraints can be arrays, slices, vectors, or iterators. `u16` also works via
	/// `Into<Constraint>`.
	pub fn new<I>(direction: Direction, constraints: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Constraint>,
	{
		Self {
			direction,
			constraints: constraints.into_iter().map(Into::into).collect(),
		}
	}

	/// Creates a new vertical layout.
	pub fn vertical<I>(constraints: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Constraint>,
	{
		Self::new(Direction::Vertical, constraints)
	}

	/// Creates a new horizontal layout.
	pub fn horizontal<I>(constraints: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Constraint>,
	{
		Self::new(Direction::Horizontal, constraints)
	}

	/// Sets the direction of the layout.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn direction(mut self, direction: Direction) -> Self {
		self.direction = direction;
		self
	}

	/// Sets the constraints.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn constraints<I>(mut self, constraints: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Constraint>,
	{
		self.constraints = constraints.into_iter().map(Into::into).collect();
		self
	}

	/// Splits into N sub-rects. Panics if constraint count != N.
	///
	/// Use [`Self::split`] when the number of areas is only known at runtime.
	pub fn areas<const N: usize>(&self, area: Rect) -> [Rect; N] {
		let rects = self.split(area);
		rects
			.as_ref()
			.try_into()
			.unwrap_or_else(|_| panic!("expected {N} rects, got {}", rects.len()))
	}

	/// Splits area into sub-rects using a deterministic algorithm.
	///
	/// Priority: Length > Percentage > Min (gets remainder).
	pub fn split(&self, area: Rect) -> Rc<[Rect]> {
		let total = match self.direction {
			Direction::Horizontal => area.width,
			Direction::Vertical => area.height,
		} as i32;

		let n = self.constraints.len();
		if n == 0 {
			return Rc::from([]);
		}

		let mut sizes = vec![0i32; n];
		let mut remaining = total;

		// Pass 1: allocate Length constraints exactly
		for (i, c) in self.constraints.iter().enumerate() {
			if let Constraint::Length(len) = c {
				let alloc = (*len as i32).min(remaining.max(0));
				sizes[i] = alloc;
				remaining -= alloc;
			}
		}

		// Pass 2: allocate Percentage constraints
		for (i, c) in self.constraints.iter().enumerate() {
			if let Constraint::Percentage(pct) = c {
				let alloc = ((total as i64 * *pct as i64) / 100) as i32;
				let alloc = alloc.min(remaining.max(0));
				sizes[i] = alloc;
				remaining -= alloc;
			}
		}

		// Pass 3: distribute remainder to Min constraints
		let min_count = self
			.constraints
			.iter()
			.filter(|c| matches!(c, Constraint::Min(_)))
			.count();
		if min_count > 0 {
			// First ensure minimums
			for (i, c) in self.constraints.iter().enumerate() {
				if let Constraint::Min(min) = c {
					let alloc = (*min as i32).min(remaining.max(0));
					sizes[i] = alloc;
					remaining -= alloc;
				}
			}
			// Then distribute remaining evenly among Min constraints
			if remaining > 0 {
				let per_min = remaining / min_count as i32;
				let mut extra = remaining % min_count as i32;
				for (i, c) in self.constraints.iter().enumerate() {
					if matches!(c, Constraint::Min(_)) {
						sizes[i] += per_min;
						if extra > 0 {
							sizes[i] += 1;
							extra -= 1;
						}
					}
				}
			}
		}

		// Clamp negative sizes to 0
		for s in &mut sizes {
			*s = (*s).max(0);
		}

		// Build rects
		let mut pos = match self.direction {
			Direction::Horizontal => area.x,
			Direction::Vertical => area.y,
		};

		let rects: Vec<Rect> = sizes
			.iter()
			.map(|&size| {
				let rect = match self.direction {
					Direction::Horizontal => Rect::new(pos, area.y, size as u16, area.height),
					Direction::Vertical => Rect::new(area.x, pos, area.width, size as u16),
				};
				pos += size as u16;
				rect
			})
			.collect();

		Rc::from(rects)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::layout::{Constraint, Rect};

	#[test]
	fn vertical_min_length() {
		let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]);
		let [content, status] = layout.areas(Rect::new(0, 0, 80, 24));
		assert_eq!(content, Rect::new(0, 0, 80, 23));
		assert_eq!(status, Rect::new(0, 23, 80, 1));
	}

	#[test]
	fn vertical_min_length_tiny() {
		let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]);
		let [content, status] = layout.areas(Rect::new(0, 0, 80, 1));
		assert_eq!(status, Rect::new(0, 0, 80, 1));
		assert_eq!(content, Rect::new(0, 0, 80, 0));
	}

	#[test]
	fn vertical_pct_min_pct() {
		let layout = Layout::vertical([
			Constraint::Percentage(25),
			Constraint::Min(1),
			Constraint::Percentage(10),
		]);
		let [top, mid, bot] = layout.areas(Rect::new(0, 0, 80, 100));
		assert_eq!(top.height, 25);
		assert_eq!(bot.height, 10);
		assert_eq!(mid.height, 65);
	}

	#[test]
	fn horizontal_split() {
		let layout = Layout::horizontal([
			Constraint::Percentage(25),
			Constraint::Min(1),
			Constraint::Percentage(25),
		]);
		let [left, mid, right] = layout.areas(Rect::new(0, 0, 80, 24));
		assert_eq!(left.width, 20);
		assert_eq!(right.width, 20);
		assert_eq!(mid.width, 40);
	}

	#[test]
	fn length_zero_collapses() {
		let layout = Layout::vertical([
			Constraint::Length(0),
			Constraint::Min(1),
			Constraint::Length(0),
		]);
		let [top, mid, bot] = layout.areas(Rect::new(0, 0, 80, 24));
		assert_eq!(top.height, 0);
		assert_eq!(bot.height, 0);
		assert_eq!(mid.height, 24);
	}

	#[test]
	fn sum_equals_total() {
		let layout = Layout::vertical([
			Constraint::Percentage(30),
			Constraint::Min(1),
			Constraint::Length(5),
		]);
		let rects = layout.split(Rect::new(0, 0, 80, 50));
		let total_height: u16 = rects.iter().map(|r| r.height).sum();
		assert_eq!(total_height, 50);
	}

	#[test]
	fn contiguous_no_gaps() {
		let layout = Layout::vertical([
			Constraint::Percentage(30),
			Constraint::Min(1),
			Constraint::Length(5),
		]);
		let rects = layout.split(Rect::new(0, 0, 80, 50));
		for pair in rects.windows(2) {
			assert_eq!(pair[0].y + pair[0].height, pair[1].y);
		}
	}

	#[test]
	fn zero_height_area() {
		let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]);
		let [content, status] = layout.areas(Rect::new(0, 0, 80, 0));
		assert_eq!(content.height, 0);
		assert_eq!(status.height, 0);
	}
}
