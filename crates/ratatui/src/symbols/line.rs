/// Vertical line.
pub const VERTICAL: &str = "│";
/// Double vertical line.
pub const DOUBLE_VERTICAL: &str = "║";
/// Thick vertical line.
pub const THICK_VERTICAL: &str = "┃";
/// Light double-dashed vertical line.
pub const LIGHT_DOUBLE_DASH_VERTICAL: &str = "╎";
/// Heavy double-dashed vertical line.
pub const HEAVY_DOUBLE_DASH_VERTICAL: &str = "╏";
/// Light triple-dashed vertical line.
pub const LIGHT_TRIPLE_DASH_VERTICAL: &str = "┆";
/// Heavy triple-dashed vertical line.
pub const HEAVY_TRIPLE_DASH_VERTICAL: &str = "┇";
/// Light quadruple-dashed vertical line.
pub const LIGHT_QUADRUPLE_DASH_VERTICAL: &str = "┊";
/// Heavy quadruple-dashed vertical line.
pub const HEAVY_QUADRUPLE_DASH_VERTICAL: &str = "┋";

/// Horizontal line.
pub const HORIZONTAL: &str = "─";
/// Double horizontal line.
pub const DOUBLE_HORIZONTAL: &str = "═";
/// Thick horizontal line.
pub const THICK_HORIZONTAL: &str = "━";
/// Light double-dashed horizontal line.
pub const LIGHT_DOUBLE_DASH_HORIZONTAL: &str = "╌";
/// Heavy double-dashed horizontal line.
pub const HEAVY_DOUBLE_DASH_HORIZONTAL: &str = "╍";
/// Light triple-dashed horizontal line.
pub const LIGHT_TRIPLE_DASH_HORIZONTAL: &str = "┄";
/// Heavy triple-dashed horizontal line.
pub const HEAVY_TRIPLE_DASH_HORIZONTAL: &str = "┅";
/// Light quadruple-dashed horizontal line.
pub const LIGHT_QUADRUPLE_DASH_HORIZONTAL: &str = "┈";
/// Heavy quadruple-dashed horizontal line.
pub const HEAVY_QUADRUPLE_DASH_HORIZONTAL: &str = "┉";

/// Top right corner.
pub const TOP_RIGHT: &str = "┐";
/// Rounded top right corner.
pub const ROUNDED_TOP_RIGHT: &str = "╮";
/// Double top right corner.
pub const DOUBLE_TOP_RIGHT: &str = "╗";
/// Thick top right corner.
pub const THICK_TOP_RIGHT: &str = "┓";

/// Top left corner.
pub const TOP_LEFT: &str = "┌";
/// Rounded top left corner.
pub const ROUNDED_TOP_LEFT: &str = "╭";
/// Double top left corner.
pub const DOUBLE_TOP_LEFT: &str = "╔";
/// Thick top left corner.
pub const THICK_TOP_LEFT: &str = "┏";

/// Bottom right corner.
pub const BOTTOM_RIGHT: &str = "┘";
/// Rounded bottom right corner.
pub const ROUNDED_BOTTOM_RIGHT: &str = "╯";
/// Double bottom right corner.
pub const DOUBLE_BOTTOM_RIGHT: &str = "╝";
/// Thick bottom right corner.
pub const THICK_BOTTOM_RIGHT: &str = "┛";

/// Bottom left corner.
pub const BOTTOM_LEFT: &str = "└";
/// Rounded bottom left corner.
pub const ROUNDED_BOTTOM_LEFT: &str = "╰";
/// Double bottom left corner.
pub const DOUBLE_BOTTOM_LEFT: &str = "╚";
/// Thick bottom left corner.
pub const THICK_BOTTOM_LEFT: &str = "┗";

/// Vertical line with a left branch.
pub const VERTICAL_LEFT: &str = "┤";
/// Double vertical line with a left branch.
pub const DOUBLE_VERTICAL_LEFT: &str = "╣";
/// Thick vertical line with a left branch.
pub const THICK_VERTICAL_LEFT: &str = "┫";

/// Vertical line with a right branch.
pub const VERTICAL_RIGHT: &str = "├";
/// Double vertical line with a right branch.
pub const DOUBLE_VERTICAL_RIGHT: &str = "╠";
/// Thick vertical line with a right branch.
pub const THICK_VERTICAL_RIGHT: &str = "┣";

/// Horizontal line with a downward branch.
pub const HORIZONTAL_DOWN: &str = "┬";
/// Double horizontal line with a downward branch.
pub const DOUBLE_HORIZONTAL_DOWN: &str = "╦";
/// Thick horizontal line with a downward branch.
pub const THICK_HORIZONTAL_DOWN: &str = "┳";

/// Horizontal line with an upward branch.
pub const HORIZONTAL_UP: &str = "┴";
/// Double horizontal line with an upward branch.
pub const DOUBLE_HORIZONTAL_UP: &str = "╩";
/// Thick horizontal line with an upward branch.
pub const THICK_HORIZONTAL_UP: &str = "┻";

/// Cross symbol.
pub const CROSS: &str = "┼";
/// Double cross symbol.
pub const DOUBLE_CROSS: &str = "╬";
/// Thick cross symbol.
pub const THICK_CROSS: &str = "╋";

/// A set of symbols used to draw lines.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Set<'a> {
	/// Vertical line.
	pub vertical: &'a str,
	/// Horizontal line.
	pub horizontal: &'a str,
	/// Top right corner.
	pub top_right: &'a str,
	/// Top left corner.
	pub top_left: &'a str,
	/// Bottom right corner.
	pub bottom_right: &'a str,
	/// Bottom left corner.
	pub bottom_left: &'a str,
	/// Vertical line with a left branch.
	pub vertical_left: &'a str,
	/// Vertical line with a right branch.
	pub vertical_right: &'a str,
	/// Horizontal line with a downward branch.
	pub horizontal_down: &'a str,
	/// Horizontal line with an upward branch.
	pub horizontal_up: &'a str,
	/// Cross symbol.
	pub cross: &'a str,
}

impl Default for Set<'_> {
	fn default() -> Self {
		NORMAL
	}
}

/// Normal line symbols.
pub const NORMAL: Set = Set {
	vertical: VERTICAL,
	horizontal: HORIZONTAL,
	top_right: TOP_RIGHT,
	top_left: TOP_LEFT,
	bottom_right: BOTTOM_RIGHT,
	bottom_left: BOTTOM_LEFT,
	vertical_left: VERTICAL_LEFT,
	vertical_right: VERTICAL_RIGHT,
	horizontal_down: HORIZONTAL_DOWN,
	horizontal_up: HORIZONTAL_UP,
	cross: CROSS,
};

/// Rounded line symbols.
pub const ROUNDED: Set = Set {
	top_right: ROUNDED_TOP_RIGHT,
	top_left: ROUNDED_TOP_LEFT,
	bottom_right: ROUNDED_BOTTOM_RIGHT,
	bottom_left: ROUNDED_BOTTOM_LEFT,
	..NORMAL
};

/// Double line symbols.
pub const DOUBLE: Set = Set {
	vertical: DOUBLE_VERTICAL,
	horizontal: DOUBLE_HORIZONTAL,
	top_right: DOUBLE_TOP_RIGHT,
	top_left: DOUBLE_TOP_LEFT,
	bottom_right: DOUBLE_BOTTOM_RIGHT,
	bottom_left: DOUBLE_BOTTOM_LEFT,
	vertical_left: DOUBLE_VERTICAL_LEFT,
	vertical_right: DOUBLE_VERTICAL_RIGHT,
	horizontal_down: DOUBLE_HORIZONTAL_DOWN,
	horizontal_up: DOUBLE_HORIZONTAL_UP,
	cross: DOUBLE_CROSS,
};

/// Thick line symbols.
pub const THICK: Set = Set {
	vertical: THICK_VERTICAL,
	horizontal: THICK_HORIZONTAL,
	top_right: THICK_TOP_RIGHT,
	top_left: THICK_TOP_LEFT,
	bottom_right: THICK_BOTTOM_RIGHT,
	bottom_left: THICK_BOTTOM_LEFT,
	vertical_left: THICK_VERTICAL_LEFT,
	vertical_right: THICK_VERTICAL_RIGHT,
	horizontal_down: THICK_HORIZONTAL_DOWN,
	horizontal_up: THICK_HORIZONTAL_UP,
	cross: THICK_CROSS,
};

/// Light double-dashed line symbols.
pub const LIGHT_DOUBLE_DASHED: Set = Set {
	vertical: LIGHT_DOUBLE_DASH_VERTICAL,
	horizontal: LIGHT_DOUBLE_DASH_HORIZONTAL,
	..NORMAL
};

/// Heavy double-dashed line symbols.
pub const HEAVY_DOUBLE_DASHED: Set = Set {
	vertical: HEAVY_DOUBLE_DASH_VERTICAL,
	horizontal: HEAVY_DOUBLE_DASH_HORIZONTAL,
	..THICK
};

/// Light triple-dashed line symbols.
pub const LIGHT_TRIPLE_DASHED: Set = Set {
	vertical: LIGHT_TRIPLE_DASH_VERTICAL,
	horizontal: LIGHT_TRIPLE_DASH_HORIZONTAL,
	..NORMAL
};

/// Heavy triple-dashed line symbols.
pub const HEAVY_TRIPLE_DASHED: Set = Set {
	vertical: HEAVY_TRIPLE_DASH_VERTICAL,
	horizontal: HEAVY_TRIPLE_DASH_HORIZONTAL,
	..THICK
};

/// Light quadruple-dashed line symbols.
pub const LIGHT_QUADRUPLE_DASHED: Set = Set {
	vertical: LIGHT_QUADRUPLE_DASH_VERTICAL,
	horizontal: LIGHT_QUADRUPLE_DASH_HORIZONTAL,
	..NORMAL
};

/// Heavy quadruple-dashed line symbols.
pub const HEAVY_QUADRUPLE_DASHED: Set = Set {
	vertical: HEAVY_QUADRUPLE_DASH_VERTICAL,
	horizontal: HEAVY_QUADRUPLE_DASH_HORIZONTAL,
	..THICK
};

#[cfg(test)]
mod tests {
	use alloc::format;
	use alloc::string::String;

	use indoc::{formatdoc, indoc};

	use super::*;

	#[test]
	fn default() {
		assert_eq!(Set::default(), NORMAL);
	}

	/// A helper function to render a set of symbols.
	fn render(set: Set) -> String {
		formatdoc!(
			"{}{}{}{}
             {}{}{}{}
             {}{}{}{}
             {}{}{}{}",
			set.top_left,
			set.horizontal,
			set.horizontal_down,
			set.top_right,
			set.vertical,
			" ",
			set.vertical,
			set.vertical,
			set.vertical_right,
			set.horizontal,
			set.cross,
			set.vertical_left,
			set.bottom_left,
			set.horizontal,
			set.horizontal_up,
			set.bottom_right
		)
	}

	#[test]
	fn normal() {
		assert_eq!(
			render(NORMAL),
			indoc!(
				"┌─┬┐
                 │ ││
                 ├─┼┤
                 └─┴┘"
			)
		);
	}

	#[test]
	fn rounded() {
		assert_eq!(
			render(ROUNDED),
			indoc!(
				"╭─┬╮
                 │ ││
                 ├─┼┤
                 ╰─┴╯"
			)
		);
	}

	#[test]
	fn double() {
		assert_eq!(
			render(DOUBLE),
			indoc!(
				"╔═╦╗
                 ║ ║║
                 ╠═╬╣
                 ╚═╩╝"
			)
		);
	}

	#[test]
	fn thick() {
		assert_eq!(
			render(THICK),
			indoc!(
				"┏━┳┓
                 ┃ ┃┃
                 ┣━╋┫
                 ┗━┻┛"
			)
		);
	}
}
