/// Defines the direction of a layout.
///
/// This enumeration is used with [`Layout`](crate::layout::Layout) to specify whether layout
/// segments should be arranged horizontally or vertically.
///
/// - `Horizontal`: Layout segments are arranged side by side (left to right)
/// - `Vertical`: Layout segments are arranged top to bottom (default)
///
/// For comprehensive layout documentation and examples, see the [`layout`](crate::layout) module.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Direction {
	/// Layout segments are arranged side by side (left to right).
	Horizontal,
	/// Layout segments are arranged top to bottom (default).
	#[default]
	Vertical,
}

impl Direction {
	/// The perpendicular direction to this direction.
	///
	/// `Horizontal` returns `Vertical`, and `Vertical` returns `Horizontal`.
	#[inline]
	#[must_use = "returns the perpendicular direction"]
	pub const fn perpendicular(self) -> Self {
		match self {
			Self::Horizontal => Self::Vertical,
			Self::Vertical => Self::Horizontal,
		}
	}
}

impl std::fmt::Display for Direction {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Horizontal => write!(f, "Horizontal"),
			Self::Vertical => write!(f, "Vertical"),
		}
	}
}

impl std::str::FromStr for Direction {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Horizontal" => Ok(Self::Horizontal),
			"Vertical" => Ok(Self::Vertical),
			_ => Err(format!("unknown variant: {s}")),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn direction_to_string() {
		assert_eq!(Direction::Horizontal.to_string(), "Horizontal");
		assert_eq!(Direction::Vertical.to_string(), "Vertical");
	}

	#[test]
	fn direction_from_str() {
		assert_eq!("Horizontal".parse::<Direction>(), Ok(Direction::Horizontal));
		assert_eq!("Vertical".parse::<Direction>(), Ok(Direction::Vertical));
		assert!("".parse::<Direction>().is_err());
	}

	#[test]
	fn other() {
		use Direction::*;
		assert_eq!(Horizontal.perpendicular(), Vertical);
		assert_eq!(Vertical.perpendicular(), Horizontal);
	}
}
