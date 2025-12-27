use crate::symbols::{block, line};

/// Scrollbar Set
/// ```text
/// <--▮------->
/// ^  ^   ^   ^
/// │  │   │   └ end
/// │  │   └──── track
/// │  └──────── thumb
/// └─────────── begin
/// ```
/// A set of symbols used to draw a scrollbar.
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Set<'a> {
	/// Track symbol.
	pub track: &'a str,
	/// Thumb symbol.
	pub thumb: &'a str,
	/// Begin symbol.
	pub begin: &'a str,
	/// End symbol.
	pub end: &'a str,
}

/// Double vertical scrollbar symbols.
pub const DOUBLE_VERTICAL: Set = Set {
	track: line::DOUBLE_VERTICAL,
	thumb: block::FULL,
	begin: "▲",
	end: "▼",
};

/// Double horizontal scrollbar symbols.
pub const DOUBLE_HORIZONTAL: Set = Set {
	track: line::DOUBLE_HORIZONTAL,
	thumb: block::FULL,
	begin: "◄",
	end: "►",
};

/// Vertical scrollbar symbols.
pub const VERTICAL: Set = Set {
	track: line::VERTICAL,
	thumb: block::FULL,
	begin: "↑",
	end: "↓",
};

/// Horizontal scrollbar symbols.
pub const HORIZONTAL: Set = Set {
	track: line::HORIZONTAL,
	thumb: block::FULL,
	begin: "←",
	end: "→",
};
