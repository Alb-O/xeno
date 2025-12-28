use alloc::borrow::Cow;
use alloc::string::ToString;
use core::fmt;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::{Style, Styled};
use crate::text::{Line, StyledGrapheme};
use crate::widgets::Widget;

/// Represents a part of a line that is contiguous and where all characters share the same style.
///
/// A `Span` is the smallest unit of text that can be styled. It is usually combined in the [`Line`]
/// type to represent a line of text where each `Span` may have a different style.
///
/// # Constructor Methods
///
/// - [`Span::default`] creates an span with empty content and the default style.
/// - [`Span::raw`] creates an span with the specified content and the default style.
/// - [`Span::styled`] creates an span with the specified content and style.
///
/// # Setter Methods
///
/// These methods are fluent setters. They return a new `Span` with the specified property set.
///
/// - [`Span::content`] sets the content of the span.
/// - [`Span::style`] sets the style of the span.
///
/// # Other Methods
///
/// - [`Span::patch_style`] patches the style of the span, adding modifiers from the given style.
/// - [`Span::reset_style`] resets the style of the span.
/// - [`Span::width`] returns the unicode width of the content held by this span.
/// - [`Span::styled_graphemes`] returns an iterator over the graphemes held by this span.
///
/// # Examples
///
/// A `Span` with `style` set to [`Style::default()`] can be created from a `&str`, a `String`, or
/// any type convertible to [`Cow<str>`].
///
/// ```rust
/// use evildoer_tui::text::Span;
///
/// let span = Span::raw("test content");
/// let span = Span::raw(String::from("test content"));
/// let span = Span::from("test content");
/// let span = Span::from(String::from("test content"));
/// let span: Span = "test content".into();
/// let span: Span = String::from("test content").into();
/// ```
///
/// Styled spans can be created using [`Span::styled`] or by converting strings using methods from
/// the [`Stylize`] trait.
///
/// ```rust
/// use evildoer_tui::style::{Style, Stylize};
/// use evildoer_tui::text::Span;
///
/// let span = Span::styled("test content", Style::new().green());
/// let span = Span::styled(String::from("test content"), Style::new().green());
///
/// // using Stylize trait shortcuts
/// let span = "test content".green();
/// let span = String::from("test content").green();
/// ```
///
/// `Span` implements the [`Styled`] trait, which allows it to be styled using the shortcut methods
/// defined in the [`Stylize`] trait.
///
/// ```rust
/// use evildoer_tui::style::Stylize;
/// use evildoer_tui::text::Span;
///
/// let span = Span::raw("test content").green().on_yellow().italic();
/// let span = Span::raw(String::from("test content"))
///     .green()
///     .on_yellow()
///     .italic();
/// ```
///
/// `Span` implements the [`Widget`] trait, which allows it to be rendered to a [`Buffer`]. Often
/// apps will use the `Paragraph` widget instead of rendering `Span` directly, as it handles text
/// wrapping and alignment for you.
///
/// ```rust,ignore
/// use evildoer_tui::{style::Stylize, Frame};
///
/// # fn render_frame(frame: &mut Frame) {
/// frame.render_widget("test content".green().on_yellow().italic(), frame.area());
/// # }
/// ```
/// [`Line`]: crate::text::Line
/// [`Stylize`]: crate::style::Stylize
/// [`Cow<str>`]: std::borrow::Cow
#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct Span<'a> {
	/// The style of the span.
	pub style: Style,
	/// The content of the span as a Clone-on-write string.
	pub content: Cow<'a, str>,
}

impl fmt::Debug for Span<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.content.is_empty() {
			write!(f, "Span::default()")?;
		} else {
			write!(f, "Span::from({:?})", self.content)?;
		}
		if self.style != Style::default() {
			self.style.fmt_stylize(f)?;
		}
		Ok(())
	}
}

impl<'a> Span<'a> {
	/// Create a span with the default style.
	///
	/// # Examples
	///
	/// ```rust
	/// use evildoer_tui::text::Span;
	///
	/// Span::raw("test content");
	/// Span::raw(String::from("test content"));
	/// ```
	pub fn raw<T>(content: T) -> Self
	where
		T: Into<Cow<'a, str>>,
	{
		Self {
			content: content.into(),
			style: Style::default(),
		}
	}

	/// Create a span with the specified style.
	///
	/// `content` accepts any type that is convertible to [`Cow<str>`] (e.g. `&str`, `String`,
	/// `&String`, etc.).
	///
	/// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
	/// your own type that implements [`Into<Style>`]).
	///
	/// # Examples
	///
	/// ```rust
	/// use evildoer_tui::style::{Style, Stylize};
	/// use evildoer_tui::text::Span;
	///
	/// let style = Style::new().yellow().on_green().italic();
	/// Span::styled("test content", style);
	/// Span::styled(String::from("test content"), style);
	/// ```
	///
	/// [`Color`]: crate::style::Color
	pub fn styled<T, S>(content: T, style: S) -> Self
	where
		T: Into<Cow<'a, str>>,
		S: Into<Style>,
	{
		Self {
			content: content.into(),
			style: style.into(),
		}
	}

	/// Sets the content of the span.
	///
	/// This is a fluent setter method which must be chained or used as it consumes self
	///
	/// Accepts any type that can be converted to [`Cow<str>`] (e.g. `&str`, `String`, `&String`,
	/// etc.).
	///
	/// # Examples
	///
	/// ```rust
	/// use evildoer_tui::text::Span;
	///
	/// let mut span = Span::default().content("content");
	/// ```
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn content<T>(mut self, content: T) -> Self
	where
		T: Into<Cow<'a, str>>,
	{
		self.content = content.into();
		self
	}

	/// Sets the style of the span.
	///
	/// This is a fluent setter method which must be chained or used as it consumes self
	///
	/// In contrast to [`Span::patch_style`], this method replaces the style of the span instead of
	/// patching it.
	///
	/// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
	/// your own type that implements [`Into<Style>`]).
	///
	/// # Examples
	///
	/// ```rust
	/// use evildoer_tui::style::{Style, Stylize};
	/// use evildoer_tui::text::Span;
	///
	/// let mut span = Span::default().style(Style::new().green());
	/// ```
	///
	/// [`Color`]: crate::style::Color
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
		self.style = style.into();
		self
	}

	/// Patches the style of the Span, adding modifiers from the given style.
	///
	/// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
	/// your own type that implements [`Into<Style>`]).
	///
	/// This is a fluent setter method which must be chained or used as it consumes self
	///
	/// # Example
	///
	/// ```rust
	/// use evildoer_tui::style::{Style, Stylize};
	/// use evildoer_tui::text::Span;
	///
	/// let span = Span::styled("test content", Style::new().green().italic())
	///     .patch_style(Style::new().red().on_yellow().bold());
	/// assert_eq!(span.style, Style::new().red().on_yellow().italic().bold());
	/// ```
	///
	/// [`Color`]: crate::style::Color
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn patch_style<S: Into<Style>>(mut self, style: S) -> Self {
		self.style = self.style.patch(style);
		self
	}

	/// Resets the style of the Span.
	///
	/// This is Equivalent to calling `patch_style(Style::reset())`.
	///
	/// This is a fluent setter method which must be chained or used as it consumes self
	///
	/// # Example
	///
	/// ```rust
	/// use evildoer_tui::style::{Style, Stylize};
	/// use evildoer_tui::text::Span;
	///
	/// let span = Span::styled(
	///     "Test Content",
	///     Style::new().dark_gray().on_yellow().italic(),
	/// )
	/// .reset_style();
	/// assert_eq!(span.style, Style::reset());
	/// ```
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn reset_style(self) -> Self {
		self.patch_style(Style::reset())
	}

	/// Returns the unicode width of the content held by this span.
	pub fn width(&self) -> usize {
		UnicodeWidthStr::width(self)
	}

	/// Returns an iterator over the graphemes held by this span.
	///
	/// `base_style` is the [`Style`] that will be patched with the `Span`'s `style` to get the
	/// resulting [`Style`].
	///
	/// `base_style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`],
	/// or your own type that implements [`Into<Style>`]).
	///
	/// # Example
	///
	/// ```rust
	/// use std::iter::Iterator;
	///
	/// use evildoer_tui::style::{Style, Stylize};
	/// use evildoer_tui::text::{Span, StyledGrapheme};
	///
	/// let span = Span::styled("Test", Style::new().green().italic());
	/// let style = Style::new().red().on_yellow();
	/// assert_eq!(
	///     span.styled_graphemes(style)
	///         .collect::<Vec<StyledGrapheme>>(),
	///     vec![
	///         StyledGrapheme::new("T", Style::new().green().on_yellow().italic()),
	///         StyledGrapheme::new("e", Style::new().green().on_yellow().italic()),
	///         StyledGrapheme::new("s", Style::new().green().on_yellow().italic()),
	///         StyledGrapheme::new("t", Style::new().green().on_yellow().italic()),
	///     ],
	/// );
	/// ```
	///
	/// [`Color`]: crate::style::Color
	pub fn styled_graphemes<S: Into<Style>>(
		&'a self,
		base_style: S,
	) -> impl Iterator<Item = StyledGrapheme<'a>> {
		let style = base_style.into().patch(self.style);
		self.content
			.as_ref()
			.graphemes(true)
			.filter(|g| !g.contains(char::is_control))
			.map(move |g| StyledGrapheme { symbol: g, style })
	}

	/// Converts this Span into a left-aligned [`Line`]
	///
	/// # Example
	///
	/// ```rust
	/// use evildoer_tui::style::Stylize;
	///
	/// let line = "Test Content".green().italic().into_left_aligned_line();
	/// ```
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn into_left_aligned_line(self) -> Line<'a> {
		Line::from(self).left_aligned()
	}

	#[expect(clippy::wrong_self_convention)]
	#[deprecated = "use `into_left_aligned_line()` instead"]
	/// Converts this Span into a left-aligned [`Line`]
	pub fn to_left_aligned_line(self) -> Line<'a> {
		self.into_left_aligned_line()
	}

	/// Converts this Span into a center-aligned [`Line`]
	///
	/// # Example
	///
	/// ```rust
	/// use evildoer_tui::style::Stylize;
	///
	/// let line = "Test Content".green().italic().into_centered_line();
	/// ```
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn into_centered_line(self) -> Line<'a> {
		Line::from(self).centered()
	}

	#[expect(clippy::wrong_self_convention)]
	#[deprecated = "use `into_centered_line()` instead"]
	/// Converts this Span into a center-aligned [`Line`]
	pub fn to_centered_line(self) -> Line<'a> {
		self.into_centered_line()
	}

	/// Converts this Span into a right-aligned [`Line`]
	///
	/// # Example
	///
	/// ```rust
	/// use evildoer_tui::style::Stylize;
	///
	/// let line = "Test Content".green().italic().into_right_aligned_line();
	/// ```
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn into_right_aligned_line(self) -> Line<'a> {
		Line::from(self).right_aligned()
	}

	#[expect(clippy::wrong_self_convention)]
	#[deprecated = "use `into_right_aligned_line()` instead"]
	/// Converts this Span into a right-aligned [`Line`]
	pub fn to_right_aligned_line(self) -> Line<'a> {
		self.into_right_aligned_line()
	}
}

impl UnicodeWidthStr for Span<'_> {
	fn width(&self) -> usize {
		self.content.width()
	}

	fn width_cjk(&self) -> usize {
		self.content.width_cjk()
	}
}

impl<'a, T> From<T> for Span<'a>
where
	T: Into<Cow<'a, str>>,
{
	fn from(s: T) -> Self {
		Span::raw(s.into())
	}
}

impl<'a> core::ops::Add<Self> for Span<'a> {
	type Output = Line<'a>;

	fn add(self, rhs: Self) -> Self::Output {
		Line::from_iter([self, rhs])
	}
}

impl Styled for Span<'_> {
	type Item = Self;

	fn style(&self) -> Style {
		self.style
	}

	fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
		self.style(style)
	}
}

impl Widget for Span<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		Widget::render(&self, area, buf);
	}
}

impl Widget for &Span<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let area = area.intersection(buf.area);
		if area.is_empty() {
			return;
		}
		let Rect { mut x, y, .. } = area;
		for (i, grapheme) in self.styled_graphemes(Style::default()).enumerate() {
			let symbol_width = grapheme.symbol.width();
			let next_x = x.saturating_add(symbol_width as u16);
			if next_x > area.right() {
				break;
			}

			if i == 0 {
				// the first grapheme is always set on the cell
				buf[(x, y)]
					.set_symbol(grapheme.symbol)
					.set_style(grapheme.style);
			} else if x == area.x {
				// there is one or more zero-width graphemes in the first cell, so the first cell
				// must be appended to.
				buf[(x, y)]
					.append_symbol(grapheme.symbol)
					.set_style(grapheme.style);
			} else if symbol_width == 0 {
				// append zero-width graphemes to the previous cell
				buf[(x - 1, y)]
					.append_symbol(grapheme.symbol)
					.set_style(grapheme.style);
			} else {
				// just a normal grapheme (not first, not zero-width, not overflowing the area)
				buf[(x, y)]
					.set_symbol(grapheme.symbol)
					.set_style(grapheme.style);
			}

			// multi-width graphemes must clear the cells of characters that are hidden by the
			// grapheme, otherwise the hidden characters will be re-rendered if the grapheme is
			// overwritten.
			for x_hidden in (x + 1)..next_x {
				// it may seem odd that the style of the hidden cells are not set to the style of
				// the grapheme, but this is how the existing buffer.set_span() method works.
				buf[(x_hidden, y)].reset();
			}
			x = next_x;
		}
	}
}

/// A trait for converting a value to a [`Span`].
///
/// This trait is automatically implemented for any type that implements the [`Display`] trait. As
/// such, `ToSpan` shouln't be implemented directly: [`Display`] should be implemented instead, and
/// you get the `ToSpan` implementation for free.
///
/// [`Display`]: std::fmt::Display
pub trait ToSpan {
	/// Converts the value to a [`Span`].
	fn to_span(&self) -> Span<'_>;
}

/// # Panics
///
/// In this implementation, the `to_span` method panics if the `Display` implementation returns an
/// error. This indicates an incorrect `Display` implementation since `fmt::Write for String` never
/// returns an error itself.
impl<T: fmt::Display> ToSpan for T {
	fn to_span(&self) -> Span<'_> {
		Span::raw(self.to_string())
	}
}

impl fmt::Display for Span<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for line in self.content.lines() {
			fmt::Display::fmt(line, f)?;
		}
		Ok(())
	}
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
