//! Dirty flags for redraw aggregation.

use std::ops::{BitOr, BitOrAssign};

/// Indicates what needs redrawing after message application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dirty(u8);

impl Dirty {
	/// No redraw needed.
	pub const NONE: Self = Self(0);

	/// Full redraw required.
	pub const FULL: Self = Self(1);

	/// Returns true if any redraw is needed.
	#[inline]
	pub fn needs_redraw(self) -> bool {
		self.0 != 0
	}
}

impl BitOr for Dirty {
	type Output = Self;

	fn bitor(self, rhs: Self) -> Self::Output {
		Self(self.0 | rhs.0)
	}
}

impl BitOrAssign for Dirty {
	fn bitor_assign(&mut self, rhs: Self) {
		self.0 |= rhs.0;
	}
}
