use core::fmt;

/// A constraint that defines the size of a layout element.
///
/// Constraints are the core mechanism for defining how space should be allocated within a
/// [`Layout`](crate::layout::Layout). They can specify fixed sizes (length), proportional sizes
/// (percentage), or minimum sizes for layout elements. Relative constraints (percentage) are
/// calculated relative to the entire space being divided.
///
/// Constraints are prioritized in the following order:
///
/// 1. [`Constraint::Length`] - allocated first (exact size)
/// 2. [`Constraint::Percentage`] - allocated second (proportional)
/// 3. [`Constraint::Min`] - receives remaining space
///
/// # Size Calculation
///
/// The deterministic solver allocates space in priority passes:
/// - Pass 1: `Length` constraints receive their exact value (clamped to remaining space).
/// - Pass 2: `Percentage` constraints receive their proportional share (clamped to remaining).
/// - Pass 3: `Min` constraints receive at least their minimum, then split any leftover evenly.
///
/// # Collection Creation
///
/// - [`from_lengths`](Self::from_lengths) - Create a collection of length constraints
/// - [`from_percentages`](Self::from_percentages) - Create a collection of percentage constraints
/// - [`from_mins`](Self::from_mins) - Create a collection of minimum constraints
///
/// # Conversion and Construction
///
/// - [`from(u16)`](Self::from) - Create a [`Length`](Self::Length) constraint from `u16`
/// - [`from(&Constraint)`](Self::from) - Create from `&Constraint` (copy)
/// - [`as_ref()`](Self::as_ref) - Get a reference to self
/// - [`default()`](Self::default) - Create default constraint
///   ([`Percentage(100)`](Self::Percentage))
///
/// # Examples
///
/// `Constraint` provides helper methods to create lists of constraints from various input formats.
///
/// ```rust
/// use xeno_tui::layout::Constraint;
///
/// // Create a layout with specified lengths for each element
/// let constraints = Constraint::from_lengths([10, 20, 10]);
///
/// // Create a centered layout using percentage constraints
/// let constraints = Constraint::from_percentages([25, 50, 25]);
///
/// // Create a centered layout with a minimum size constraint for specific elements
/// let constraints = Constraint::from_mins([0, 100, 0]);
/// ```
///
/// For comprehensive layout documentation and examples, see the [`layout`](crate::layout) module.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Constraint {
	/// Applies a minimum size constraint to the element.
	///
	/// The element size is set to at least the specified amount. Any remaining space after
	/// `Length` and `Percentage` constraints are satisfied is distributed evenly among `Min`
	/// constraints (after satisfying their minimums).
	///
	/// # Examples
	///
	/// `[Percentage(100), Min(20)]`
	///
	/// ```plain
	/// ┌────────────────────────────┐┌──────────────────┐
	/// │            30 px           ││       20 px      │
	/// └────────────────────────────┘└──────────────────┘
	/// ```
	///
	/// `[Percentage(100), Min(10)]`
	///
	/// ```plain
	/// ┌──────────────────────────────────────┐┌────────┐
	/// │                 40 px                ││  10 px │
	/// └──────────────────────────────────────┘└────────┘
	/// ```
	Min(u16),

	/// Applies a length constraint to the element.
	///
	/// The element size is set to the specified amount exactly.
	///
	/// # Examples
	///
	/// `[Length(20), Length(20)]`
	///
	/// ```plain
	/// ┌──────────────────┐┌──────────────────┐
	/// │       20 px      ││       20 px      │
	/// └──────────────────┘└──────────────────┘
	/// ```
	///
	/// `[Length(20), Length(30)]`
	///
	/// ```plain
	/// ┌──────────────────┐┌────────────────────────────┐
	/// │       20 px      ││            30 px           │
	/// └──────────────────┘└────────────────────────────┘
	/// ```
	Length(u16),

	/// Applies a percentage of the available space to the element.
	///
	/// Converts the given percentage to a floating-point value and multiplies that with the total
	/// area. This value is rounded back to an integer as part of the layout split calculation.
	///
	/// # Examples
	///
	/// `[Percentage(75), Min(1)]`
	///
	/// ```plain
	/// ┌────────────────────────────────────┐┌──────────┐
	/// │                38 px               ││   12 px  │
	/// └────────────────────────────────────┘└──────────┘
	/// ```
	///
	/// `[Percentage(50), Min(1)]`
	///
	/// ```plain
	/// ┌───────────────────────┐┌───────────────────────┐
	/// │         25 px         ││         25 px         │
	/// └───────────────────────┘└───────────────────────┘
	/// ```
	Percentage(u16),
}

impl Constraint {
	/// Convert an iterator of lengths into a vector of constraints.
	///
	/// # Examples
	///
	/// ```rust
	/// use xeno_tui::layout::{Constraint, Layout, Rect};
	///
	/// # let area = Rect::default();
	/// let constraints = Constraint::from_lengths([1, 2, 3]);
	/// let layout = Layout::default().constraints(constraints).split(area);
	/// ```
	pub fn from_lengths<T>(lengths: T) -> Vec<Self>
	where
		T: IntoIterator<Item = u16>,
	{
		lengths.into_iter().map(Self::Length).collect()
	}

	/// Convert an iterator of percentages into a vector of constraints.
	///
	/// # Examples
	///
	/// ```rust
	/// use xeno_tui::layout::{Constraint, Layout, Rect};
	///
	/// # let area = Rect::default();
	/// let constraints = Constraint::from_percentages([25, 50, 25]);
	/// let layout = Layout::default().constraints(constraints).split(area);
	/// ```
	pub fn from_percentages<T>(percentages: T) -> Vec<Self>
	where
		T: IntoIterator<Item = u16>,
	{
		percentages.into_iter().map(Self::Percentage).collect()
	}

	/// Convert an iterator of mins into a vector of constraints.
	///
	/// # Examples
	///
	/// ```rust
	/// use xeno_tui::layout::{Constraint, Layout, Rect};
	///
	/// # let area = Rect::default();
	/// let constraints = Constraint::from_mins([1, 2, 3]);
	/// let layout = Layout::default().constraints(constraints).split(area);
	/// ```
	pub fn from_mins<T>(mins: T) -> Vec<Self>
	where
		T: IntoIterator<Item = u16>,
	{
		mins.into_iter().map(Self::Min).collect()
	}
}

impl From<u16> for Constraint {
	/// Convert a `u16` into a [`Constraint::Length`].
	///
	/// This is useful when you want to specify a fixed size for a layout, but don't want to
	/// explicitly create a [`Constraint::Length`] yourself.
	///
	/// # Examples
	///
	/// ```rust
	/// use xeno_tui::layout::{Constraint, Direction, Layout, Rect};
	///
	/// # let area = Rect::default();
	/// let layout = Layout::new(Direction::Vertical, [1, 2, 3]).split(area);
	/// let layout = Layout::horizontal([1, 2, 3]).split(area);
	/// let layout = Layout::vertical([1, 2, 3]).split(area);
	/// ````
	fn from(length: u16) -> Self {
		Self::Length(length)
	}
}

impl From<&Self> for Constraint {
	fn from(constraint: &Self) -> Self {
		*constraint
	}
}

impl AsRef<Self> for Constraint {
	fn as_ref(&self) -> &Self {
		self
	}
}

impl Default for Constraint {
	fn default() -> Self {
		Self::Percentage(100)
	}
}

impl fmt::Display for Constraint {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Percentage(p) => write!(f, "Percentage({p})"),
			Self::Length(l) => write!(f, "Length({l})"),
			Self::Min(m) => write!(f, "Min({m})"),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn default() {
		assert_eq!(Constraint::default(), Constraint::Percentage(100));
	}

	#[test]
	fn to_string() {
		assert_eq!(Constraint::Percentage(50).to_string(), "Percentage(50)");
		assert_eq!(Constraint::Length(10).to_string(), "Length(10)");
		assert_eq!(Constraint::Min(10).to_string(), "Min(10)");
	}

	#[test]
	fn from_lengths() {
		let expected = [
			Constraint::Length(1),
			Constraint::Length(2),
			Constraint::Length(3),
		];
		assert_eq!(Constraint::from_lengths([1, 2, 3]), expected);
		assert_eq!(Constraint::from_lengths(vec![1, 2, 3]), expected);
	}

	#[test]
	fn from_percentages() {
		let expected = [
			Constraint::Percentage(25),
			Constraint::Percentage(50),
			Constraint::Percentage(25),
		];
		assert_eq!(Constraint::from_percentages([25, 50, 25]), expected);
		assert_eq!(Constraint::from_percentages(vec![25, 50, 25]), expected);
	}

	#[test]
	fn from_mins() {
		let expected = [Constraint::Min(1), Constraint::Min(2), Constraint::Min(3)];
		assert_eq!(Constraint::from_mins([1, 2, 3]), expected);
		assert_eq!(Constraint::from_mins(vec![1, 2, 3]), expected);
	}
}
