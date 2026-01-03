//! Implementation of [`Screen`] and [`Cell`] traits for `vt100` types.

use super::widget::{Cell, Screen};
use crate::style::{Modifier, Style};

impl Screen for vt100::Screen {
	type C = vt100::Cell;

	#[inline]
	fn cell(&self, row: u16, col: u16) -> Option<&Self::C> {
		self.cell(row, col)
	}

	#[inline]
	fn hide_cursor(&self) -> bool {
		self.hide_cursor()
	}

	#[inline]
	fn cursor_position(&self) -> (u16, u16) {
		self.cursor_position()
	}
}

impl Cell for vt100::Cell {
	#[inline]
	fn has_contents(&self) -> bool {
		self.has_contents()
	}

	#[inline]
	fn apply(&self, cell: &mut crate::buffer::Cell) {
		fill_buf_cell(self, cell)
	}
}

/// Copies cell content and styling from a vt100 screen cell to a buffer cell.
#[inline]
fn fill_buf_cell(screen_cell: &vt100::Cell, buf_cell: &mut crate::buffer::Cell) {
	let fg = screen_cell.fgcolor();
	let bg = screen_cell.bgcolor();
	if screen_cell.has_contents() {
		buf_cell.set_symbol(&screen_cell.contents());
	}
	let fg: Color = fg.into();
	let bg: Color = bg.into();
	let mut style = Style::reset();
	if screen_cell.bold() {
		style = style.add_modifier(Modifier::BOLD);
	}
	if screen_cell.italic() {
		style = style.add_modifier(Modifier::ITALIC);
	}
	if screen_cell.underline() {
		style = style.add_modifier(Modifier::UNDERLINED);
	}
	if screen_cell.inverse() {
		style = style.add_modifier(Modifier::REVERSED);
	}
	buf_cell.set_style(style);
	buf_cell.set_fg(fg.into());
	buf_cell.set_bg(bg.into());
}

/// Intermediate color type for converting between `vt100::Color` and `xeno_tui::style::Color`.
#[allow(dead_code, reason = "variants used for color conversion lookup")]
enum Color {
	/// Default/reset color.
	Reset,
	/// ANSI black (index 0).
	Black,
	/// ANSI red (index 1).
	Red,
	/// ANSI green (index 2).
	Green,
	/// ANSI yellow (index 3).
	Yellow,
	/// ANSI blue (index 4).
	Blue,
	/// ANSI magenta (index 5).
	Magenta,
	/// ANSI cyan (index 6).
	Cyan,
	/// ANSI gray/white (index 7).
	Gray,
	/// ANSI bright black/dark gray (index 8).
	DarkGray,
	/// ANSI bright red (index 9).
	LightRed,
	/// ANSI bright green (index 10).
	LightGreen,
	/// ANSI bright yellow (index 11).
	LightYellow,
	/// ANSI bright blue (index 12).
	LightBlue,
	/// ANSI bright magenta (index 13).
	LightMagenta,
	/// ANSI bright cyan (index 14).
	LightCyan,
	/// ANSI bright white (index 15).
	White,
	/// 24-bit true color (red, green, blue).
	Rgb(u8, u8, u8),
	/// 256-color palette index.
	Indexed(u8),
}

impl From<vt100::Color> for Color {
	#[inline]
	fn from(value: vt100::Color) -> Self {
		match value {
			vt100::Color::Default => Self::Reset,
			vt100::Color::Idx(i) => Self::Indexed(i),
			vt100::Color::Rgb(r, g, b) => Self::Rgb(r, g, b),
		}
	}
}

impl From<Color> for vt100::Color {
	#[inline]
	fn from(value: Color) -> Self {
		match value {
			Color::Reset => Self::Default,
			Color::Black => Self::Idx(0),
			Color::Red => Self::Idx(1),
			Color::Green => Self::Idx(2),
			Color::Yellow => Self::Idx(3),
			Color::Blue => Self::Idx(4),
			Color::Magenta => Self::Idx(5),
			Color::Cyan => Self::Idx(6),
			Color::Gray => Self::Idx(7),
			Color::DarkGray => Self::Idx(8),
			Color::LightRed => Self::Idx(9),
			Color::LightGreen => Self::Idx(10),
			Color::LightYellow => Self::Idx(11),
			Color::LightBlue => Self::Idx(12),
			Color::LightMagenta => Self::Idx(13),
			Color::LightCyan => Self::Idx(14),
			Color::White => Self::Idx(15),
			Color::Rgb(r, g, b) => Self::Rgb(r, g, b),
			Color::Indexed(i) => Self::Idx(i),
		}
	}
}

impl From<Color> for crate::style::Color {
	#[inline]
	fn from(value: Color) -> Self {
		match value {
			Color::Reset => Self::Reset,
			Color::Black => Self::Black,
			Color::Red => Self::Red,
			Color::Green => Self::Green,
			Color::Yellow => Self::Yellow,
			Color::Blue => Self::Blue,
			Color::Magenta => Self::Magenta,
			Color::Cyan => Self::Cyan,
			Color::Gray => Self::Gray,
			Color::DarkGray => Self::DarkGray,
			Color::LightRed => Self::LightRed,
			Color::LightGreen => Self::LightGreen,
			Color::LightYellow => Self::LightYellow,
			Color::LightBlue => Self::LightBlue,
			Color::LightMagenta => Self::LightMagenta,
			Color::LightCyan => Self::LightCyan,
			Color::White => Self::White,
			Color::Rgb(r, g, b) => Self::Rgb(r, g, b),
			Color::Indexed(i) => Self::Indexed(i),
		}
	}
}
