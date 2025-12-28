use crate::symbols::{block, line};

/// A set of symbols used to draw a border.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Set<'a> {
	/// Top left corner.
	pub top_left: &'a str,
	/// Top right corner.
	pub top_right: &'a str,
	/// Bottom left corner.
	pub bottom_left: &'a str,
	/// Bottom right corner.
	pub bottom_right: &'a str,
	/// Vertical line on the left.
	pub vertical_left: &'a str,
	/// Vertical line on the right.
	pub vertical_right: &'a str,
	/// Horizontal line on the top.
	pub horizontal_top: &'a str,
	/// Horizontal line on the bottom.
	pub horizontal_bottom: &'a str,
}

impl Default for Set<'_> {
	fn default() -> Self {
		PLAIN
	}
}

// Helper function to convert a line set to a border set
const fn from_line_set(line_set: line::Set<'_>) -> Set<'_> {
	Set {
		top_left: line_set.top_left,
		top_right: line_set.top_right,
		bottom_left: line_set.bottom_left,
		bottom_right: line_set.bottom_right,
		vertical_left: line_set.vertical,
		vertical_right: line_set.vertical,
		horizontal_top: line_set.horizontal,
		horizontal_bottom: line_set.horizontal,
	}
}

/// Border Set with a single line width
///
/// ```text
/// ┌─────┐
/// │xxxxx│
/// │xxxxx│
/// └─────┘
/// ```
pub const PLAIN: Set = Set {
	top_left: line::NORMAL.top_left,
	top_right: line::NORMAL.top_right,
	bottom_left: line::NORMAL.bottom_left,
	bottom_right: line::NORMAL.bottom_right,
	vertical_left: line::NORMAL.vertical,
	vertical_right: line::NORMAL.vertical,
	horizontal_top: line::NORMAL.horizontal,
	horizontal_bottom: line::NORMAL.horizontal,
};

/// Border Set with a single line width and rounded corners
///
/// ```text
/// ╭─────╮
/// │xxxxx│
/// │xxxxx│
/// ╰─────╯
/// ```
pub const ROUNDED: Set = Set {
	top_left: line::ROUNDED.top_left,
	top_right: line::ROUNDED.top_right,
	bottom_left: line::ROUNDED.bottom_left,
	bottom_right: line::ROUNDED.bottom_right,
	vertical_left: line::ROUNDED.vertical,
	vertical_right: line::ROUNDED.vertical,
	horizontal_top: line::ROUNDED.horizontal,
	horizontal_bottom: line::ROUNDED.horizontal,
};

/// Border Set with a double line width
///
/// ```text
/// ╔═════╗
/// ║xxxxx║
/// ║xxxxx║
/// ╚═════╝
/// ```
pub const DOUBLE: Set = Set {
	top_left: line::DOUBLE.top_left,
	top_right: line::DOUBLE.top_right,
	bottom_left: line::DOUBLE.bottom_left,
	bottom_right: line::DOUBLE.bottom_right,
	vertical_left: line::DOUBLE.vertical,
	vertical_right: line::DOUBLE.vertical,
	horizontal_top: line::DOUBLE.horizontal,
	horizontal_bottom: line::DOUBLE.horizontal,
};

/// Border Set with a thick line width
///
/// ```text
/// ┏━━━━━┓
/// ┃xxxxx┃
/// ┃xxxxx┃
/// ┗━━━━━┛
/// ```
pub const THICK: Set = Set {
	top_left: line::THICK.top_left,
	top_right: line::THICK.top_right,
	bottom_left: line::THICK.bottom_left,
	bottom_right: line::THICK.bottom_right,
	vertical_left: line::THICK.vertical,
	vertical_right: line::THICK.vertical,
	horizontal_top: line::THICK.horizontal,
	horizontal_bottom: line::THICK.horizontal,
};

/// Border Set with light double-dashed border lines
///
/// ```text
/// ┌╌╌╌╌╌┐
/// ╎xxxxx╎
/// ╎xxxxx╎
/// └╌╌╌╌╌┘
/// ```
pub const LIGHT_DOUBLE_DASHED: Set = from_line_set(line::LIGHT_DOUBLE_DASHED);

/// Border Set with thick double-dashed border lines
///
/// ```text
/// ┏╍╍╍╍╍┓
/// ╏xxxxx╏
/// ╏xxxxx╏
/// ┗╍╍╍╍╍┛
/// ```
pub const HEAVY_DOUBLE_DASHED: Set = from_line_set(line::HEAVY_DOUBLE_DASHED);

/// Border Set with light triple-dashed border lines
///
/// ```text
/// ┌┄┄┄┄┄┐
/// ┆xxxxx┆
/// ┆xxxxx┆
/// └┄┄┄┄┄┘
/// ```
pub const LIGHT_TRIPLE_DASHED: Set = from_line_set(line::LIGHT_TRIPLE_DASHED);

/// Border Set with thick triple-dashed border lines
///
/// ```text
/// ┏┅┅┅┅┅┓
/// ┇xxxxx┇
/// ┇xxxxx┇
/// ┗┅┅┅┅┅┛
/// ```
pub const HEAVY_TRIPLE_DASHED: Set = from_line_set(line::HEAVY_TRIPLE_DASHED);

/// Border Set with light quadruple-dashed border lines
///
/// ```text
/// ┌┈┈┈┈┈┐
/// ┊xxxxx┊
/// ┊xxxxx┊
/// └┈┈┈┈┈┘
/// ```
pub const LIGHT_QUADRUPLE_DASHED: Set = from_line_set(line::LIGHT_QUADRUPLE_DASHED);

/// Border Set with thick quadruple-dashed border lines
///
/// ```text
/// ┏┉┉┉┉┉┓
/// ┋xxxxx┋
/// ┋xxxxx┋
/// ┗┉┉┉┉┉┛
/// ```
pub const HEAVY_QUADRUPLE_DASHED: Set = from_line_set(line::HEAVY_QUADRUPLE_DASHED);

/// Top left quadrant.
pub const QUADRANT_TOP_LEFT: &str = "▘";
/// Top right quadrant.
pub const QUADRANT_TOP_RIGHT: &str = "▝";
/// Bottom left quadrant.
pub const QUADRANT_BOTTOM_LEFT: &str = "▖";
/// Bottom right quadrant.
pub const QUADRANT_BOTTOM_RIGHT: &str = "▗";
/// Top half quadrant.
pub const QUADRANT_TOP_HALF: &str = "▀";
/// Bottom half quadrant.
pub const QUADRANT_BOTTOM_HALF: &str = "▄";
/// Left half quadrant.
pub const QUADRANT_LEFT_HALF: &str = "▌";
/// Right half quadrant.
pub const QUADRANT_RIGHT_HALF: &str = "▐";
/// Top-left, bottom-left, and bottom-right quadrant.
pub const QUADRANT_TOP_LEFT_BOTTOM_LEFT_BOTTOM_RIGHT: &str = "▙";
/// Top-left, top-right, and bottom-left quadrant.
pub const QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_LEFT: &str = "▛";
/// Top-left, top-right, and bottom-right quadrant.
pub const QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_RIGHT: &str = "▜";
/// Top-right, bottom-left, and bottom-right quadrant.
pub const QUADRANT_TOP_RIGHT_BOTTOM_LEFT_BOTTOM_RIGHT: &str = "▟";
/// Top-left and bottom-right quadrant.
pub const QUADRANT_TOP_LEFT_BOTTOM_RIGHT: &str = "▚";
/// Top-right and bottom-left quadrant.
pub const QUADRANT_TOP_RIGHT_BOTTOM_LEFT: &str = "▞";
/// Full block quadrant.
pub const QUADRANT_BLOCK: &str = "█";

/// Quadrant used for setting a border outside a block by one half cell "pixel".
///
/// ```text
/// ▛▀▀▀▀▀▜
/// ▌xxxxx▐
/// ▌xxxxx▐
/// ▙▄▄▄▄▄▟
/// ```
pub const QUADRANT_OUTSIDE: Set = Set {
	top_left: QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_LEFT,
	top_right: QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_RIGHT,
	bottom_left: QUADRANT_TOP_LEFT_BOTTOM_LEFT_BOTTOM_RIGHT,
	bottom_right: QUADRANT_TOP_RIGHT_BOTTOM_LEFT_BOTTOM_RIGHT,
	vertical_left: QUADRANT_LEFT_HALF,
	vertical_right: QUADRANT_RIGHT_HALF,
	horizontal_top: QUADRANT_TOP_HALF,
	horizontal_bottom: QUADRANT_BOTTOM_HALF,
};

/// Quadrant used for setting a border inside a block by one half cell "pixel".
///
/// ```text
/// ▗▄▄▄▄▄▖
/// ▐xxxxx▌
/// ▐xxxxx▌
/// ▝▀▀▀▀▀▘
/// ```
pub const QUADRANT_INSIDE: Set = Set {
	top_right: QUADRANT_BOTTOM_LEFT,
	top_left: QUADRANT_BOTTOM_RIGHT,
	bottom_right: QUADRANT_TOP_LEFT,
	bottom_left: QUADRANT_TOP_RIGHT,
	vertical_left: QUADRANT_RIGHT_HALF,
	vertical_right: QUADRANT_LEFT_HALF,
	horizontal_top: QUADRANT_BOTTOM_HALF,
	horizontal_bottom: QUADRANT_TOP_HALF,
};

/// Top eight of a cell.
pub const ONE_EIGHTH_TOP_EIGHT: &str = "▔";
/// Bottom eight of a cell.
pub const ONE_EIGHTH_BOTTOM_EIGHT: &str = "▁";
/// Left eight of a cell.
pub const ONE_EIGHTH_LEFT_EIGHT: &str = "▏";
/// Right eight of a cell.
pub const ONE_EIGHTH_RIGHT_EIGHT: &str = "▕";

/// Wide border set based on McGugan box technique
///
/// ```text
/// ▁▁▁▁▁▁▁
/// ▏xxxxx▕
/// ▏xxxxx▕
/// ▔▔▔▔▔▔▔
/// ```
#[expect(clippy::doc_markdown)]
pub const ONE_EIGHTH_WIDE: Set = Set {
	top_right: ONE_EIGHTH_BOTTOM_EIGHT,
	top_left: ONE_EIGHTH_BOTTOM_EIGHT,
	bottom_right: ONE_EIGHTH_TOP_EIGHT,
	bottom_left: ONE_EIGHTH_TOP_EIGHT,
	vertical_left: ONE_EIGHTH_LEFT_EIGHT,
	vertical_right: ONE_EIGHTH_RIGHT_EIGHT,
	horizontal_top: ONE_EIGHTH_BOTTOM_EIGHT,
	horizontal_bottom: ONE_EIGHTH_TOP_EIGHT,
};

/// Tall border set based on McGugan box technique
///
/// ```text
/// ▕▔▔▏
/// ▕xx▏
/// ▕xx▏
/// ▕▁▁▏
/// ```
#[expect(clippy::doc_markdown)]
pub const ONE_EIGHTH_TALL: Set = Set {
	top_right: ONE_EIGHTH_LEFT_EIGHT,
	top_left: ONE_EIGHTH_RIGHT_EIGHT,
	bottom_right: ONE_EIGHTH_LEFT_EIGHT,
	bottom_left: ONE_EIGHTH_RIGHT_EIGHT,
	vertical_left: ONE_EIGHTH_RIGHT_EIGHT,
	vertical_right: ONE_EIGHTH_LEFT_EIGHT,
	horizontal_top: ONE_EIGHTH_TOP_EIGHT,
	horizontal_bottom: ONE_EIGHTH_BOTTOM_EIGHT,
};

/// Wide proportional (visually equal width and height) border with using set of quadrants.
///
/// The border is created by using half blocks for top and bottom, and full
/// blocks for right and left sides to make horizontal and vertical borders seem equal.
///
/// ```text
/// ▄▄▄▄
/// █xx█
/// █xx█
/// ▀▀▀▀
/// ```
pub const PROPORTIONAL_WIDE: Set = Set {
	top_right: QUADRANT_BOTTOM_HALF,
	top_left: QUADRANT_BOTTOM_HALF,
	bottom_right: QUADRANT_TOP_HALF,
	bottom_left: QUADRANT_TOP_HALF,
	vertical_left: QUADRANT_BLOCK,
	vertical_right: QUADRANT_BLOCK,
	horizontal_top: QUADRANT_BOTTOM_HALF,
	horizontal_bottom: QUADRANT_TOP_HALF,
};

/// Tall proportional (visually equal width and height) border with using set of quadrants.
///
/// The border is created by using full blocks for all sides, except for the top and bottom,
/// which use half blocks to make horizontal and vertical borders seem equal.
///
/// ```text
/// ▕█▀▀█
/// ▕█xx█
/// ▕█xx█
/// ▕█▄▄█
/// ```
pub const PROPORTIONAL_TALL: Set = Set {
	top_right: QUADRANT_BLOCK,
	top_left: QUADRANT_BLOCK,
	bottom_right: QUADRANT_BLOCK,
	bottom_left: QUADRANT_BLOCK,
	vertical_left: QUADRANT_BLOCK,
	vertical_right: QUADRANT_BLOCK,
	horizontal_top: QUADRANT_TOP_HALF,
	horizontal_bottom: QUADRANT_BOTTOM_HALF,
};

/// Solid border set
///
/// The border is created by using full blocks for all sides.
///
/// ```text
/// ████
/// █xx█
/// █xx█
/// ████
/// ```
pub const FULL: Set = Set {
	top_left: block::FULL,
	top_right: block::FULL,
	bottom_left: block::FULL,
	bottom_right: block::FULL,
	vertical_left: block::FULL,
	vertical_right: block::FULL,
	horizontal_top: block::FULL,
	horizontal_bottom: block::FULL,
};

/// Empty border set
///
/// The border is created by using empty strings for all sides.
///
/// This is useful for ensuring that the border style is applied to a border on a block with a title
/// without actually drawing a border.
///
/// ░ Example
///
/// `░` represents the content in the area not covered by the border to make it easier to see the
/// blank symbols.
///
/// ```text
/// ░░░░░░░░
/// ░░    ░░
/// ░░ ░░ ░░
/// ░░ ░░ ░░
/// ░░    ░░
/// ░░░░░░░░
/// ```
pub const EMPTY: Set = Set {
	top_left: " ",
	top_right: " ",
	bottom_left: " ",
	bottom_right: " ",
	vertical_left: " ",
	vertical_right: " ",
	horizontal_top: " ",
	horizontal_bottom: " ",
};

/// Stripe border set
///
/// A border with a thin colored stripe on the left edge only, with empty space on all other
/// sides. This is useful for notification toasts and callout boxes where a thin accent bar
/// indicates the notification level or type.
///
/// The left edge uses a left one-eighth block character (▏) which can be styled with a
/// foreground color to create the stripe effect. This block element connects seamlessly
/// when stacked vertically. All other borders are spaces, using the block's background style.
///
/// ```text
/// ░░░░░░░░
/// ░▏    ░░
/// ░▏ ░░ ░░
/// ░▏ ░░ ░░
/// ░▏    ░░
/// ░░░░░░░░
/// ```
pub const STRIPE: Set = Set {
	top_left: block::ONE_EIGHTH,
	top_right: " ",
	bottom_left: block::ONE_EIGHTH,
	bottom_right: " ",
	vertical_left: block::ONE_EIGHTH,
	vertical_right: " ",
	horizontal_top: " ",
	horizontal_bottom: " ",
};

#[cfg(test)]
#[path = "border/tests.rs"]
mod tests;
