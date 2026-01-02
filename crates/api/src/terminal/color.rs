//! Terminal color mapping.

use evildoer_registry::panels::SplitColor;

/// Maps vt100 colors to SplitColor.
pub fn map_vt_color(color: vt100::Color) -> Option<SplitColor> {
	match color {
		vt100::Color::Default => None,
		vt100::Color::Idx(i) => Some(SplitColor::Indexed(i)),
		vt100::Color::Rgb(r, g, b) => Some(SplitColor::Rgb(r, g, b)),
	}
}
