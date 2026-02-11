/// Horizontal content alignment within a layout area.
///
/// This type is used throughout Ratatui to control how content is positioned horizontally within
/// available space. It's commonly used with widgets to control text alignment, but can also be
/// used in layout calculations.
///
/// For comprehensive layout documentation and examples, see the [`layout`](crate::layout) module.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HorizontalAlignment {
	/// Content is aligned to the left side of the area.
	#[default]
	Left,
	/// Content is centered within the area.
	Center,
	/// Content is aligned to the right side of the area.
	Right,
}

/// Vertical content alignment within a layout area.
///
/// This type is used to control how content is positioned vertically within available space.
/// It complements [`HorizontalAlignment`] to provide full 2D positioning control.
///
/// For comprehensive layout documentation and examples, see the [`layout`](crate::layout) module.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VerticalAlignment {
	/// Content is aligned to the top of the area.
	#[default]
	Top,
	/// Content is centered vertically within the area.
	Center,
	/// Content is aligned to the bottom of the area.
	Bottom,
}

impl std::fmt::Display for HorizontalAlignment {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Left => write!(f, "Left"),
			Self::Center => write!(f, "Center"),
			Self::Right => write!(f, "Right"),
		}
	}
}

impl std::str::FromStr for HorizontalAlignment {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Left" => Ok(Self::Left),
			"Center" => Ok(Self::Center),
			"Right" => Ok(Self::Right),
			_ => Err(format!("unknown variant: {s}")),
		}
	}
}

impl std::fmt::Display for VerticalAlignment {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Top => write!(f, "Top"),
			Self::Center => write!(f, "Center"),
			Self::Bottom => write!(f, "Bottom"),
		}
	}
}

impl std::str::FromStr for VerticalAlignment {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Top" => Ok(Self::Top),
			"Center" => Ok(Self::Center),
			"Bottom" => Ok(Self::Bottom),
			_ => Err(format!("unknown variant: {s}")),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn horizontal_alignment_to_string() {
		assert_eq!(HorizontalAlignment::Left.to_string(), "Left");
		assert_eq!(HorizontalAlignment::Center.to_string(), "Center");
		assert_eq!(HorizontalAlignment::Right.to_string(), "Right");
	}

	#[test]
	fn horizontal_alignment_from_str() {
		assert_eq!("Left".parse::<HorizontalAlignment>(), Ok(HorizontalAlignment::Left));
		assert_eq!("Center".parse::<HorizontalAlignment>(), Ok(HorizontalAlignment::Center));
		assert_eq!("Right".parse::<HorizontalAlignment>(), Ok(HorizontalAlignment::Right));
		assert!("".parse::<HorizontalAlignment>().is_err());
	}

	#[test]
	fn vertical_alignment_to_string() {
		assert_eq!(VerticalAlignment::Top.to_string(), "Top");
		assert_eq!(VerticalAlignment::Center.to_string(), "Center");
		assert_eq!(VerticalAlignment::Bottom.to_string(), "Bottom");
	}

	#[test]
	fn vertical_alignment_from_str() {
		let top = "Top".parse::<VerticalAlignment>();
		assert_eq!(top, Ok(VerticalAlignment::Top));

		let center = "Center".parse::<VerticalAlignment>();
		assert_eq!(center, Ok(VerticalAlignment::Center));

		let bottom = "Bottom".parse::<VerticalAlignment>();
		assert_eq!(bottom, Ok(VerticalAlignment::Bottom));

		let invalid = "".parse::<VerticalAlignment>();
		assert!(invalid.is_err());
	}
}
