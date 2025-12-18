/// Full block.
pub const FULL: &str = "█";
/// Seven eighths block.
pub const SEVEN_EIGHTHS: &str = "▉";
/// Three quarters block.
pub const THREE_QUARTERS: &str = "▊";
/// Five eighths block.
pub const FIVE_EIGHTHS: &str = "▋";
/// Half block.
pub const HALF: &str = "▌";
/// Three eighths block.
pub const THREE_EIGHTHS: &str = "▍";
/// One quarter block.
pub const ONE_QUARTER: &str = "▎";
/// One eighth block.
pub const ONE_EIGHTH: &str = "▏";

/// A set of symbols used to draw a block.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Set<'a> {
	/// Full block.
	pub full: &'a str,
	/// Seven eighths block.
	pub seven_eighths: &'a str,
	/// Three quarters block.
	pub three_quarters: &'a str,
	/// Five eighths block.
	pub five_eighths: &'a str,
	/// Half block.
	pub half: &'a str,
	/// Three eighths block.
	pub three_eighths: &'a str,
	/// One quarter block.
	pub one_quarter: &'a str,
	/// One eighth block.
	pub one_eighth: &'a str,
	/// Empty block.
	pub empty: &'a str,
}

impl Default for Set<'_> {
	fn default() -> Self {
		NINE_LEVELS
	}
}

/// Three levels of block symbols.
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

/// Nine levels of block symbols.
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
