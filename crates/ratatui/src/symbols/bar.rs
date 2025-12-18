/// Full bar.
pub const FULL: &str = "█";
/// Seven eighths bar.
pub const SEVEN_EIGHTHS: &str = "▇";
/// Three quarters bar.
pub const THREE_QUARTERS: &str = "▆";
/// Five eighths bar.
pub const FIVE_EIGHTHS: &str = "▅";
/// Half bar.
pub const HALF: &str = "▄";
/// Three eighths bar.
pub const THREE_EIGHTHS: &str = "▃";
/// One quarter bar.
pub const ONE_QUARTER: &str = "▂";
/// One eighth bar.
pub const ONE_EIGHTH: &str = "▁";

/// A set of symbols used to draw a bar.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Set<'a> {
	/// Full bar.
	pub full: &'a str,
	/// Seven eighths bar.
	pub seven_eighths: &'a str,
	/// Three quarters bar.
	pub three_quarters: &'a str,
	/// Five eighths bar.
	pub five_eighths: &'a str,
	/// Half bar.
	pub half: &'a str,
	/// Three eighths bar.
	pub three_eighths: &'a str,
	/// One quarter bar.
	pub one_quarter: &'a str,
	/// One eighth bar.
	pub one_eighth: &'a str,
	/// Empty bar.
	pub empty: &'a str,
}

impl Default for Set<'_> {
	fn default() -> Self {
		NINE_LEVELS
	}
}

/// Three levels of bar symbols.
pub const THREE_LEVELS: Set = Set {
	full: FULL,
	seven_eighths: FULL,
	three_quarters: HALF,
	five_eighths: HALF,
	half: HALF,
	three_eighths: HALF,
	one_quarter: HALF,
	one_eighth: " ",
	empty: " ",
};

/// Nine levels of bar symbols.
pub const NINE_LEVELS: Set = Set {
	full: FULL,
	seven_eighths: SEVEN_EIGHTHS,
	three_quarters: THREE_QUARTERS,
	five_eighths: FIVE_EIGHTHS,
	half: HALF,
	three_eighths: THREE_EIGHTHS,
	one_quarter: ONE_QUARTER,
	one_eighth: ONE_EIGHTH,
	empty: " ",
};
