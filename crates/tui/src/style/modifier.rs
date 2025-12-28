use core::fmt;

use bitflags::bitflags;

bitflags! {
	/// Modifier changes the way a piece of text is displayed.
	///
	/// They are bitflags so they can easily be composed.
	///
	/// `From<Modifier> for Style` is implemented so you can use `Modifier` anywhere that accepts
	/// `Into<Style>`.
	///
	/// ## Examples
	///
	/// ```rust
	/// use tome_tui::style::Modifier;
	///
	/// let m = Modifier::BOLD | Modifier::ITALIC;
	/// ```
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	#[derive(Default, Clone, Copy, Eq, PartialEq, Hash)]
	pub struct Modifier: u16 {
		/// Bold text.
		const BOLD              = 0b0000_0000_0001;
		/// Dim text.
		const DIM               = 0b0000_0000_0010;
		/// Italic text.
		const ITALIC            = 0b0000_0000_0100;
		/// Underlined text.
		const UNDERLINED        = 0b0000_0000_1000;
		/// Slow blinking text.
		const SLOW_BLINK        = 0b0000_0001_0000;
		/// Rapid blinking text.
		const RAPID_BLINK       = 0b0000_0010_0000;
		/// Reversed text.
		const REVERSED          = 0b0000_0100_0000;
		/// Hidden text.
		const HIDDEN            = 0b0000_1000_0000;
		/// Crossed out text.
		const CROSSED_OUT       = 0b0001_0000_0000;
	}
}

/// Implement the `Debug` trait for `Modifier` manually.
///
/// This will avoid printing the empty modifier as 'Borders(0x0)' and instead print it as 'NONE'.
impl fmt::Debug for Modifier {
	/// Format the modifier as `NONE` if the modifier is empty or as a list of flags separated by
	/// `|` otherwise.
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		if self.is_empty() {
			return write!(f, "NONE");
		}
		write!(f, "{}", self.0)
	}
}
