/// Represents the spacing between segments in a layout.
///
/// The `Spacing` enum is used to define the spacing between segments in a layout. It can represent
/// either positive spacing (space between segments) or negative spacing (overlap between segments).
///
/// # Variants
///
/// - `Space(u16)`: Represents positive spacing between segments. The value indicates the number of
///   cells.
/// - `Overlap(u16)`: Represents negative spacing, causing overlap between segments. The value
///   indicates the number of overlapping cells.
///
/// # Default
///
/// The default value for `Spacing` is `Space(0)`, which means no spacing or no overlap between
/// segments.
///
/// # Conversions
///
/// The `Spacing` enum can be created from different integer types:
///
/// - From `u16`: Directly converts the value to `Spacing::Space`.
/// - From `i16`: Converts negative values to `Spacing::Overlap` and non-negative values to
///   `Spacing::Space`.
/// - From `i32`: Clamps the value to the range of `i16` and converts negative values to
///   `Spacing::Overlap` and non-negative values to `Spacing::Space`.
///
/// See the [`Layout::spacing`](super::Layout::spacing) method for details on how to use this enum.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Spacing {
	/// Space between layout areas.
	Space(u16),
	/// Overlap between layout areas.
	Overlap(u16),
}

impl Default for Spacing {
	fn default() -> Self {
		Self::Space(0)
	}
}

impl From<i32> for Spacing {
	fn from(value: i32) -> Self {
		Self::from(value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16)
	}
}

impl From<u16> for Spacing {
	fn from(value: u16) -> Self {
		Self::Space(value)
	}
}

impl From<i16> for Spacing {
	fn from(value: i16) -> Self {
		if value < 0 {
			Self::Overlap(value.unsigned_abs())
		} else {
			Self::Space(value.unsigned_abs())
		}
	}
}
