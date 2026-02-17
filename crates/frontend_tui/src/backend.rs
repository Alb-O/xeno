use std::fmt::Write as FmtWrite;
use std::io;
use std::num::NonZeroU16;

use termina::Terminal;
use termina::escape::csi::{Csi, Cursor, Edit, EraseInDisplay, Mode, Sgr, SgrAttributes, SgrModifiers};
use termina::style::{ColorSpec, RgbaColor};
use unicode_width::UnicodeWidthStr;
use xeno_tui::backend::{Backend, WindowSize};
use xeno_tui::buffer::Cell;
use xeno_tui::layout::{Position, Size};

/// Backend implementation using the termina crate.
pub struct TerminaBackend<T>
where
	T: Terminal,
{
	/// The underlying terminal instance.
	terminal: T,
}

impl<T> TerminaBackend<T>
where
	T: Terminal,
{
	/// Creates a new backend wrapping the given terminal.
	pub fn new(terminal: T) -> Self {
		Self { terminal }
	}

	/// Returns a mutable reference to the underlying terminal.
	pub fn terminal_mut(&mut self) -> &mut T {
		&mut self.terminal
	}
}

impl<T> Backend for TerminaBackend<T>
where
	T: Terminal,
{
	type Error = io::Error;

	fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
	where
		I: Iterator<Item = (u16, u16, &'a Cell)>,
	{
		let mut out = String::with_capacity(4096);
		let mut last_attrs = SgrAttributes::default();
		let mut have_emitted_attrs = false;

		// Run state for coalescing contiguous same-attrs text.
		let mut run_y: u16 = 0;
		let mut run_x0: u16 = 0;
		let mut run_next_x: u16 = 0;
		let mut run_attrs = SgrAttributes::default();
		let mut run_text = String::new();
		let mut in_run = false;

		for (x, y, cell) in content {
			let attrs = attrs_from_cell(cell);
			let sym = cell.symbol();
			let sym = if sym.is_empty() { " " } else { sym };
			let w = sym.width().max(1) as u16;

			let appendable = in_run && y == run_y && x == run_next_x && attrs == run_attrs;

			if !appendable {
				if in_run {
					flush_run(&mut out, run_y, run_x0, &run_attrs, &mut last_attrs, &mut have_emitted_attrs, &run_text);
				}
				run_y = y;
				run_x0 = x;
				run_attrs = attrs;
				run_text.clear();
				in_run = true;
			}

			run_text.push_str(sym);
			run_next_x = x + w;
		}

		if in_run {
			flush_run(&mut out, run_y, run_x0, &run_attrs, &mut last_attrs, &mut have_emitted_attrs, &run_text);
		}

		#[cfg(feature = "perf")]
		tracing::debug!(
			target: "perf",
			termina_bytes_written = out.len() as u64,
		);

		self.terminal.write_all(out.as_bytes())?;
		Ok(())
	}

	fn hide_cursor(&mut self) -> io::Result<()> {
		write!(
			self.terminal,
			"{}",
			Csi::Mode(Mode::ResetDecPrivateMode(termina::escape::csi::DecPrivateMode::Code(
				termina::escape::csi::DecPrivateModeCode::ShowCursor
			)))
		)
	}

	fn show_cursor(&mut self) -> io::Result<()> {
		write!(
			self.terminal,
			"{}",
			Csi::Mode(Mode::SetDecPrivateMode(termina::escape::csi::DecPrivateMode::Code(
				termina::escape::csi::DecPrivateModeCode::ShowCursor
			)))
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
		write!(self.terminal, "{}", Csi::Edit(Edit::EraseInDisplay(EraseInDisplay::EraseDisplay)))
	}

	fn clear_region(&mut self, clear_type: xeno_tui::backend::ClearType) -> io::Result<()> {
		use xeno_tui::backend::ClearType;
		match clear_type {
			ClearType::All => self.clear(),
			ClearType::AfterCursor => write!(self.terminal, "{}", Csi::Edit(Edit::EraseInDisplay(EraseInDisplay::EraseToEndOfDisplay))),
			ClearType::BeforeCursor => write!(self.terminal, "{}", Csi::Edit(Edit::EraseInDisplay(EraseInDisplay::EraseToStartOfDisplay))),
			ClearType::CurrentLine => write!(self.terminal, "{}", Csi::Edit(Edit::EraseInLine(termina::escape::csi::EraseInLine::EraseLine))),
			ClearType::UntilNewLine => write!(
				self.terminal,
				"{}",
				Csi::Edit(Edit::EraseInLine(termina::escape::csi::EraseInLine::EraseToEndOfLine))
			),
		}
	}

	fn scroll_region_up(&mut self, _region: std::ops::Range<u16>, _amount: u16) -> io::Result<()> {
		Ok(())
	}

	fn scroll_region_down(&mut self, _region: std::ops::Range<u16>, _amount: u16) -> io::Result<()> {
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

/// Extracts SGR attributes from a TUI cell.
fn attrs_from_cell(cell: &Cell) -> SgrAttributes {
	let mut attrs = SgrAttributes::default();

	if let Some(color) = map_color(cell.fg) {
		attrs.foreground = Some(color);
	}
	if let Some(color) = map_color(cell.bg) {
		attrs.background = Some(color);
	}
	if cell.modifier.contains(xeno_tui::style::Modifier::BOLD) {
		attrs.modifiers |= SgrModifiers::INTENSITY_BOLD;
	}
	if cell.modifier.contains(xeno_tui::style::Modifier::DIM) {
		attrs.modifiers |= SgrModifiers::INTENSITY_DIM;
	}
	if cell.modifier.contains(xeno_tui::style::Modifier::ITALIC) {
		attrs.modifiers |= SgrModifiers::ITALIC;
	}

	let underline_style = if cell.underline_style != xeno_tui::style::UnderlineStyle::Reset {
		cell.underline_style
	} else if cell.modifier.contains(xeno_tui::style::Modifier::UNDERLINED) {
		xeno_tui::style::UnderlineStyle::Line
	} else {
		xeno_tui::style::UnderlineStyle::Reset
	};

	let underline_modifier = match underline_style {
		xeno_tui::style::UnderlineStyle::Reset => None,
		xeno_tui::style::UnderlineStyle::Line => Some(SgrModifiers::UNDERLINE_SINGLE),
		xeno_tui::style::UnderlineStyle::Curl => Some(SgrModifiers::UNDERLINE_CURLY),
		xeno_tui::style::UnderlineStyle::Dotted => Some(SgrModifiers::UNDERLINE_DOTTED),
		xeno_tui::style::UnderlineStyle::Dashed => Some(SgrModifiers::UNDERLINE_DASHED),
		xeno_tui::style::UnderlineStyle::DoubleLine => Some(SgrModifiers::UNDERLINE_DOUBLE),
	};

	if let Some(modifier) = underline_modifier {
		if cell.underline_color != xeno_tui::style::Color::Reset {
			attrs.underline_color = map_color(cell.underline_color);
		}
		attrs.modifiers |= modifier;
	}
	if cell.modifier.contains(xeno_tui::style::Modifier::SLOW_BLINK) {
		attrs.modifiers |= SgrModifiers::BLINK_SLOW;
	}
	if cell.modifier.contains(xeno_tui::style::Modifier::RAPID_BLINK) {
		attrs.modifiers |= SgrModifiers::BLINK_RAPID;
	}
	if cell.modifier.contains(xeno_tui::style::Modifier::REVERSED) {
		attrs.modifiers |= SgrModifiers::REVERSE;
	}
	if cell.modifier.contains(xeno_tui::style::Modifier::HIDDEN) {
		attrs.modifiers |= SgrModifiers::INVISIBLE;
	}
	if cell.modifier.contains(xeno_tui::style::Modifier::CROSSED_OUT) {
		attrs.modifiers |= SgrModifiers::STRIKE_THROUGH;
	}

	attrs
}

/// Flushes a coalesced text run to the output buffer.
///
/// Emits a cursor position, SGR change (only if attributes differ from the
/// last emitted set), and the run's text content.
fn flush_run(out: &mut String, y: u16, x0: u16, attrs: &SgrAttributes, last_attrs: &mut SgrAttributes, have_emitted: &mut bool, text: &str) {
	let line = NonZeroU16::new(y + 1).unwrap_or(NonZeroU16::MIN);
	let col = NonZeroU16::new(x0 + 1).unwrap_or(NonZeroU16::MIN);
	let _ = write!(
		out,
		"{}",
		Csi::Cursor(Cursor::Position {
			line: line.into(),
			col: col.into()
		})
	);

	if !*have_emitted || *attrs != *last_attrs {
		if attrs.is_empty() {
			let _ = write!(out, "{}", Csi::Sgr(Sgr::Reset));
		} else {
			let _ = write!(out, "{}", Csi::Sgr(Sgr::Reset));
			let _ = write!(out, "{}", Csi::Sgr(Sgr::Attributes(*attrs)));
		}
		*last_attrs = *attrs;
		*have_emitted = true;
	}

	out.push_str(text);
}

/// Maps a TUI color to a termina color specification.
fn map_color(color: xeno_tui::style::Color) -> Option<ColorSpec> {
	use xeno_tui::style::Color;
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
