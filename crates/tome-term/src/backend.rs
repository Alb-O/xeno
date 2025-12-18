use std::io;
use std::num::NonZeroU16;

use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use termina::Terminal;
use termina::escape::csi::{
	Csi, Cursor, Edit, EraseInDisplay, Mode, Sgr, SgrAttributes, SgrModifiers,
};
use termina::style::{ColorSpec, RgbaColor};

pub struct TerminaBackend<T: Terminal> {
	terminal: T,
}

impl<T: Terminal> TerminaBackend<T> {
	pub fn new(terminal: T) -> Self {
		Self { terminal }
	}

	pub fn _terminal(&self) -> &T {
		&self.terminal
	}

	pub fn terminal_mut(&mut self) -> &mut T {
		&mut self.terminal
	}
}

impl<T: Terminal> Backend for TerminaBackend<T> {
	type Error = io::Error;

	fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
	where
		I: Iterator<Item = (u16, u16, &'a Cell)>,
	{
		let mut last_y = 0;
		let mut last_x = 0;
		let mut first = true;

		for (x, y, cell) in content {
			if first || y != last_y || x != last_x + 1 {
				// Termina uses 1-based coordinates
				let line = NonZeroU16::new(y + 1).unwrap_or(NonZeroU16::MIN);
				let col = NonZeroU16::new(x + 1).unwrap_or(NonZeroU16::MIN);

				write!(
					self.terminal,
					"{}",
					Csi::Cursor(Cursor::Position {
						line: line.into(),
						col: col.into()
					})
				)?;
			}
			last_x = x;
			last_y = y;
			first = false;

			let mut attrs = SgrAttributes::default();

			if let Some(color) = map_color(cell.fg) {
				attrs.foreground = Some(color);
			}

			if let Some(color) = map_color(cell.bg) {
				attrs.background = Some(color);
			}

			if cell.modifier.contains(ratatui::style::Modifier::BOLD) {
				attrs.modifiers |= SgrModifiers::INTENSITY_BOLD;
			}
			if cell.modifier.contains(ratatui::style::Modifier::DIM) {
				attrs.modifiers |= SgrModifiers::INTENSITY_DIM;
			}
			if cell.modifier.contains(ratatui::style::Modifier::ITALIC) {
				attrs.modifiers |= SgrModifiers::ITALIC;
			}
			if cell.modifier.contains(ratatui::style::Modifier::UNDERLINED) {
				attrs.modifiers |= SgrModifiers::UNDERLINE_SINGLE;
			}
			if cell.modifier.contains(ratatui::style::Modifier::SLOW_BLINK) {
				attrs.modifiers |= SgrModifiers::BLINK_SLOW;
			}
			if cell
				.modifier
				.contains(ratatui::style::Modifier::RAPID_BLINK)
			{
				attrs.modifiers |= SgrModifiers::BLINK_RAPID;
			}
			if cell.modifier.contains(ratatui::style::Modifier::REVERSED) {
				attrs.modifiers |= SgrModifiers::REVERSE;
			}
			if cell.modifier.contains(ratatui::style::Modifier::HIDDEN) {
				attrs.modifiers |= SgrModifiers::INVISIBLE;
			}
			if cell
				.modifier
				.contains(ratatui::style::Modifier::CROSSED_OUT)
			{
				attrs.modifiers |= SgrModifiers::STRIKE_THROUGH;
			}

			write!(self.terminal, "{}", Csi::Sgr(Sgr::Reset))?;
			if !attrs.is_empty() {
				write!(self.terminal, "{}", Csi::Sgr(Sgr::Attributes(attrs)))?;
			}

			write!(self.terminal, "{}", cell.symbol())?;
		}
		Ok(())
	}

	fn hide_cursor(&mut self) -> io::Result<()> {
		write!(
			self.terminal,
			"{}",
			Csi::Mode(Mode::ResetDecPrivateMode(
				termina::escape::csi::DecPrivateMode::Code(
					termina::escape::csi::DecPrivateModeCode::ShowCursor
				)
			))
		)
	}

	fn show_cursor(&mut self) -> io::Result<()> {
		write!(
			self.terminal,
			"{}",
			Csi::Mode(Mode::SetDecPrivateMode(
				termina::escape::csi::DecPrivateMode::Code(
					termina::escape::csi::DecPrivateModeCode::ShowCursor
				)
			))
		)
	}

	fn get_cursor_position(&mut self) -> io::Result<Position> {
		Ok(Position::new(0, 0))
	}

	fn set_cursor_position<P: Into<Position>>(&mut self, pos: P) -> io::Result<()> {
		let pos = pos.into();
		let line = NonZeroU16::new(pos.y + 1).unwrap_or(NonZeroU16::MIN);
		let col = NonZeroU16::new(pos.x + 1).unwrap_or(NonZeroU16::MIN);

		write!(
			self.terminal,
			"{}",
			Csi::Cursor(Cursor::Position {
				line: line.into(),
				col: col.into()
			})
		)
	}

	fn clear(&mut self) -> io::Result<()> {
		write!(
			self.terminal,
			"{}",
			Csi::Edit(Edit::EraseInDisplay(EraseInDisplay::EraseDisplay))
		)
	}

	fn clear_region(&mut self, clear_type: ratatui::backend::ClearType) -> io::Result<()> {
		use ratatui::backend::ClearType;
		match clear_type {
			ClearType::All => self.clear(),
			ClearType::AfterCursor => write!(
				self.terminal,
				"{}",
				Csi::Edit(Edit::EraseInDisplay(EraseInDisplay::EraseToEndOfDisplay))
			),
			ClearType::BeforeCursor => write!(
				self.terminal,
				"{}",
				Csi::Edit(Edit::EraseInDisplay(EraseInDisplay::EraseToStartOfDisplay))
			),
			ClearType::CurrentLine => write!(
				self.terminal,
				"{}",
				Csi::Edit(Edit::EraseInLine(
					termina::escape::csi::EraseInLine::EraseLine
				))
			),
			ClearType::UntilNewLine => write!(
				self.terminal,
				"{}",
				Csi::Edit(Edit::EraseInLine(
					termina::escape::csi::EraseInLine::EraseToEndOfLine
				))
			),
		}
	}

	fn scroll_region_up(&mut self, _region: std::ops::Range<u16>, _amount: u16) -> io::Result<()> {
		// Not implemented for now
		Ok(())
	}

	fn scroll_region_down(
		&mut self,
		_region: std::ops::Range<u16>,
		_amount: u16,
	) -> io::Result<()> {
		// Not implemented for now
		Ok(())
	}

	fn size(&self) -> io::Result<Size> {
		let size = self.terminal.get_dimensions()?;
		Ok(Size::new(size.cols, size.rows))
	}

	fn window_size(&mut self) -> io::Result<WindowSize> {
		let size = self.terminal.get_dimensions()?;
		Ok(WindowSize {
			columns_rows: Size::new(size.cols, size.rows),
			pixels: Size::new(0, 0),
		})
	}

	fn flush(&mut self) -> io::Result<()> {
		self.terminal.flush()
	}
}

fn map_color(color: ratatui::style::Color) -> Option<ColorSpec> {
	use ratatui::style::Color;
	match color {
		Color::Reset => Some(ColorSpec::Reset),
		Color::Black => Some(ColorSpec::BLACK),
		Color::Red => Some(ColorSpec::RED),
		Color::Green => Some(ColorSpec::GREEN),
		Color::Yellow => Some(ColorSpec::YELLOW),
		Color::Blue => Some(ColorSpec::BLUE),
		Color::Magenta => Some(ColorSpec::MAGENTA),
		Color::Cyan => Some(ColorSpec::CYAN),
		Color::Gray => Some(ColorSpec::WHITE),
		Color::DarkGray => Some(ColorSpec::BRIGHT_BLACK),
		Color::LightRed => Some(ColorSpec::BRIGHT_RED),
		Color::LightGreen => Some(ColorSpec::BRIGHT_GREEN),
		Color::LightYellow => Some(ColorSpec::BRIGHT_YELLOW),
		Color::LightBlue => Some(ColorSpec::BRIGHT_BLUE),
		Color::LightMagenta => Some(ColorSpec::BRIGHT_MAGENTA),
		Color::LightCyan => Some(ColorSpec::BRIGHT_CYAN),
		Color::White => Some(ColorSpec::BRIGHT_WHITE),
		Color::Rgb(r, g, b) => Some(ColorSpec::TrueColor(RgbaColor {
			red: r,
			green: g,
			blue: b,
			alpha: 255,
		})),
		Color::Indexed(i) => Some(ColorSpec::PaletteIndex(i)),
	}
}
