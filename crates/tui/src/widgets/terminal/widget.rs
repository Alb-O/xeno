use alloc::string::String;

use super::state;
use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::{Color, Modifier, Style};
use crate::widgets::{Block, Clear, Widget};

/// A trait representing a pseudo-terminal screen.
///
/// Implementing this trait allows for backends other than `vt100` to be used
/// with the `PseudoTerminal` widget.
pub trait Screen {
	/// The type of cell this screen contains
	type C: Cell;

	/// Returns the cell at the given location if it exists.
	fn cell(&self, row: u16, col: u16) -> Option<&Self::C>;

	/// Returns whether the terminal should be hidden
	fn hide_cursor(&self) -> bool;

	/// Returns cursor position of screen.
	///
	/// The return value is expected to be (row, column)
	fn cursor_position(&self) -> (u16, u16);
}

/// A trait for representing a single cell on a screen.
pub trait Cell {
	/// Whether the cell has any contents that could be rendered to the screen.
	fn has_contents(&self) -> bool;

	/// Apply the contents and styling of this cell to the provided buffer cell.
	fn apply(&self, cell: &mut crate::buffer::Cell);
}

/// A widget representing a pseudo-terminal screen.
///
/// The `PseudoTerminal` widget displays the contents of a pseudo-terminal screen,
/// which is typically populated with text and control sequences from a terminal emulator.
/// It provides a visual representation of the terminal output within a TUI application.
///
/// The contents of the pseudo-terminal screen are represented by a `vt100::Screen` object.
/// The `vt100` library provides functionality for parsing and processing terminal control
/// sequences and handling terminal state, allowing the `PseudoTerminal` widget to accurately
/// render the terminal output.
///
/// # Example
///
/// ```rust,ignore
/// use evildoer_tui::{
///     style::{Color, Modifier, Style},
///     widgets::{Block, Borders},
/// };
/// use evildoer_tui::widgets::terminal::PseudoTerminal;
/// use vt100::Parser;
///
/// let mut parser = vt100::Parser::new(24, 80, 0);
/// let pseudo_term = PseudoTerminal::new(parser.screen())
///     .block(Block::default().title("Terminal").borders(Borders::ALL))
///     .style(
///         Style::default()
///             .fg(Color::White)
///             .bg(Color::Black)
///             .add_modifier(Modifier::BOLD),
///     );
/// ```
#[non_exhaustive]
pub struct PseudoTerminal<'a, S> {
	/// Reference to the terminal screen backend.
	screen: &'a S,
	/// Optional block wrapper for borders and title.
	pub(crate) block: Option<Block<'a>>,
	/// Optional base style for the terminal content.
	style: Option<Style>,
	/// Cursor configuration and appearance.
	pub(crate) cursor: Cursor,
}

/// Configuration for the cursor in a [`PseudoTerminal`].
#[non_exhaustive]
pub struct Cursor {
	/// Whether the cursor is visible.
	pub(crate) show: bool,
	/// The character/symbol used to render the cursor.
	pub(crate) symbol: String,
	/// Style applied to the cursor when over empty space.
	pub(crate) style: Style,
	/// Style applied when the cursor overlaps existing content.
	pub(crate) overlay_style: Style,
}

impl Cursor {
	/// Sets the symbol for the cursor.
	///
	/// # Arguments
	///
	/// * `symbol`: The symbol to set as the cursor.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use evildoer_tui::style::Style;
	/// use evildoer_tui::widgets::terminal::Cursor;
	///
	/// let cursor = Cursor::default().symbol("|");
	/// ```
	#[inline]
	#[must_use]
	pub fn symbol(mut self, symbol: &str) -> Self {
		self.symbol = symbol.into();
		self
	}

	/// Sets the style for the cursor.
	///
	/// # Arguments
	///
	/// * `style`: The `Style` to set for the cursor.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use evildoer_tui::style::Style;
	/// use evildoer_tui::widgets::terminal::Cursor;
	///
	/// let cursor = Cursor::default().style(Style::default());
	/// ```
	#[inline]
	#[must_use]
	pub const fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Sets the overlay style for the cursor.
	///
	/// The overlay style is used when the cursor overlaps with existing content on the screen.
	///
	/// # Arguments
	///
	/// * `overlay_style`: The `Style` to set as the overlay style for the cursor.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use evildoer_tui::style::Style;
	/// use evildoer_tui::widgets::terminal::Cursor;
	///
	/// let cursor = Cursor::default().overlay_style(Style::default());
	/// ```
	#[inline]
	#[must_use]
	pub const fn overlay_style(mut self, overlay_style: Style) -> Self {
		self.overlay_style = overlay_style;
		self
	}

	/// Set the visibility of the cursor (default = shown)
	#[inline]
	#[must_use]
	pub const fn visibility(mut self, show: bool) -> Self {
		self.show = show;
		self
	}

	/// Show the cursor (default)
	#[inline]
	pub fn show(&mut self) {
		self.show = true;
	}

	/// Hide the cursor
	#[inline]
	pub fn hide(&mut self) {
		self.show = false;
	}
}

impl Default for Cursor {
	#[inline]
	fn default() -> Self {
		Self {
			show: true,
			symbol: "\u{2588}".into(),
			style: Style::default().fg(Color::Gray),
			overlay_style: Style::default().add_modifier(Modifier::REVERSED),
		}
	}
}

impl<'a, S: Screen> PseudoTerminal<'a, S> {
	/// Creates a new instance of `PseudoTerminal`.
	///
	/// # Arguments
	///
	/// * `screen`: The reference to the `Screen`.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use evildoer_tui::widgets::terminal::PseudoTerminal;
	/// use vt100::Parser;
	///
	/// let mut parser = vt100::Parser::new(24, 80, 0);
	/// let pseudo_term = PseudoTerminal::new(parser.screen());
	/// ```
	#[inline]
	#[must_use]
	pub fn new(screen: &'a S) -> Self {
		PseudoTerminal {
			screen,
			block: None,
			style: None,
			cursor: Cursor::default(),
		}
	}

	/// Sets the block for the `PseudoTerminal`.
	///
	/// # Arguments
	///
	/// * `block`: The `Block` to set.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use evildoer_tui::widgets::Block;
	/// use evildoer_tui::widgets::terminal::PseudoTerminal;
	/// use vt100::Parser;
	///
	/// let mut parser = vt100::Parser::new(24, 80, 0);
	/// let block = Block::default();
	/// let pseudo_term = PseudoTerminal::new(parser.screen()).block(block);
	/// ```
	#[inline]
	#[must_use]
	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	/// Sets the cursor configuration for the `PseudoTerminal`.
	///
	/// The `cursor` method allows configuring the appearance of the cursor within the
	/// `PseudoTerminal` widget.
	///
	/// # Arguments
	///
	/// * `cursor`: The `Cursor` configuration to set.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use evildoer_tui::style::Style;
	/// use evildoer_tui::widgets::terminal::{Cursor, PseudoTerminal};
	///
	/// let mut parser = vt100::Parser::new(24, 80, 0);
	/// let cursor = Cursor::default().symbol("|").style(Style::default());
	/// let pseudo_term = PseudoTerminal::new(parser.screen()).cursor(cursor);
	/// ```
	#[inline]
	#[must_use]
	pub fn cursor(mut self, cursor: Cursor) -> Self {
		self.cursor = cursor;
		self
	}

	/// Sets the style for `PseudoTerminal`.
	///
	/// # Arguments
	///
	/// * `style`: The `Style` to set.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use evildoer_tui::style::Style;
	/// use evildoer_tui::widgets::terminal::PseudoTerminal;
	///
	/// let mut parser = vt100::Parser::new(24, 80, 0);
	/// let style = Style::default();
	/// let pseudo_term = PseudoTerminal::new(parser.screen()).style(style);
	/// ```
	#[inline]
	#[must_use]
	pub const fn style(mut self, style: Style) -> Self {
		self.style = Some(style);
		self
	}

	/// Returns a reference to the underlying screen.
	#[inline]
	#[must_use]
	pub const fn screen(&self) -> &S {
		self.screen
	}
}

impl<S: Screen> Widget for PseudoTerminal<'_, S> {
	#[inline]
	fn render(self, area: Rect, buf: &mut Buffer) {
		Clear.render(area, buf);
		let area = self.block.as_ref().map_or(area, |b| {
			let inner_area = b.inner(area);
			b.clone().render(area, buf);
			inner_area
		});
		state::handle(&self, area, buf);
	}
}
