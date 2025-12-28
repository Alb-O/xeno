//! Primitives for styled text.
//!
//! A terminal UI is at its root a lot of strings. In order to make it accessible and stylish, those
//! strings may be associated to a set of styles. `tome_tui` has three ways to represent them:
//! - A single line string where all graphemes have the same style is represented by a [`Span`].
//! - A single line string where each grapheme may have its own style is represented by [`Line`].
//! - A multiple line string where each grapheme may have its own style is represented by a
//!   [`Text`].
//!
//! These types form a hierarchy: [`Line`] is a collection of [`Span`] and each line of [`Text`] is
//! a [`Line`].
//!
//! Keep it mind that a lot of widgets will use those types to advertise what kind of string is
//! supported for their properties. Moreover, `tome_tui` provides convenient `From` implementations
//! so that you can start by using simple `String` or `&str` and then promote them to the previous
//! primitives when you need additional styling capabilities.
//!
//! For example, for the `Block` widget, all the following calls are valid to set its `title`
//! property (which is a [`Line`] under the hood):
//!
//! ```rust,ignore
//! use crate::{
//!     style::{Color, Style},
//!     text::{Line, Span},
//!     widgets::Block,
//! };
//!
//! // A simple string with no styling.
//! // Converted to Line(vec![
//! //   Span { content: Cow::Borrowed("My title"), style: Style { .. } }
//! // ])
//! let block = Block::new().title("My title");
//!
//! // A simple string with a unique style.
//! // Converted to Line(vec![
//! //   Span { content: Cow::Borrowed("My title"), style: Style { fg: Some(Color::Yellow), .. }
//! // ])
//! let block = Block::new().title(Span::styled("My title", Style::default().fg(Color::Yellow)));
//!
//! // A string with multiple styles.
//! // Converted to Line(vec![
//! //   Span { content: Cow::Borrowed("My"), style: Style { fg: Some(Color::Yellow), .. } },
//! //   Span { content: Cow::Borrowed(" title"), .. }
//! // ])
//! let block = Block::new().title(vec![
//!     Span::styled("My", Style::default().fg(Color::Yellow)),
//!     Span::raw(" title"),
//! ]);
//! ```

#![warn(missing_docs)]

use alloc::borrow::{Cow, ToOwned};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use unicode_width::UnicodeWidthStr;

use crate::buffer::Buffer;
use crate::layout::{Alignment, Rect};
use crate::style::{Style, Styled};
use crate::widgets::Widget;

mod grapheme;
pub use grapheme::StyledGrapheme;

mod line;
pub use line::{Line, ToLine};

mod masked;
pub use masked::Masked;

mod span;
pub use span::{Span, ToSpan};

/// A string split over one or more [`Line`]s.
///
/// Rendered top to bottom. Implements [`Widget`] for direct rendering, or use with
/// [`Paragraph`](crate::widgets::Paragraph). Implements [`Styled`] for style shorthand.
///
/// # Example
///
/// ```rust
/// use crate::style::Stylize;
/// use crate::text::{Line, Text};
///
/// let text = Text::from("Line 1\nLine 2").yellow().italic();
/// let text = Text::from(vec![Line::from("Line 1"), Line::from("Line 2")]).centered();
/// ```
#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct Text<'a> {
	/// The alignment of this text.
	pub alignment: Option<Alignment>,
	/// The style of this text.
	pub style: Style,
	/// The lines that make up this piece of text.
	pub lines: Vec<Line<'a>>,
}

impl fmt::Debug for Text<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.lines.is_empty() {
			f.write_str("Text::default()")?;
		} else if self.lines.len() == 1 {
			write!(f, "Text::from({:?})", self.lines[0])?;
		} else {
			f.write_str("Text::from_iter(")?;
			f.debug_list().entries(self.lines.iter()).finish()?;
			f.write_str(")")?;
		}
		self.style.fmt_stylize(f)?;
		match self.alignment {
			Some(Alignment::Left) => f.write_str(".left_aligned()")?,
			Some(Alignment::Center) => f.write_str(".centered()")?,
			Some(Alignment::Right) => f.write_str(".right_aligned()")?,
			_ => (),
		}
		Ok(())
	}
}

impl<'a> Text<'a> {
	/// Create text (potentially multiple lines) with no style.
	pub fn raw<T>(content: T) -> Self
	where
		T: Into<Cow<'a, str>>,
	{
		let lines: Vec<_> = match content.into() {
			Cow::Borrowed("") => vec![Line::from("")],
			Cow::Borrowed(s) => s.lines().map(Line::from).collect(),
			Cow::Owned(s) if s.is_empty() => vec![Line::from("")],
			Cow::Owned(s) => s.lines().map(|l| Line::from(l.to_owned())).collect(),
		};
		Self::from(lines)
	}

	/// Create text with a style.
	pub fn styled<T, S>(content: T, style: S) -> Self
	where
		T: Into<Cow<'a, str>>,
		S: Into<Style>,
	{
		Self::raw(content).patch_style(style)
	}

	/// Returns the max width of all lines.
	pub fn width(&self) -> usize {
		UnicodeWidthStr::width(self)
	}

	/// Returns the number of lines.
	pub fn height(&self) -> usize {
		self.lines.len()
	}

	/// Sets the style.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
		self.style = style.into();
		self
	}

	/// Adds modifiers from the given style without overwriting existing style.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn patch_style<S: Into<Style>>(mut self, style: S) -> Self {
		self.style = self.style.patch(style);
		self
	}

	/// Resets the style to [`Style::reset()`].
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn reset_style(self) -> Self {
		self.patch_style(Style::reset())
	}

	/// Sets the alignment. Individual lines can override.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn alignment(self, alignment: Alignment) -> Self {
		Self {
			alignment: Some(alignment),
			..self
		}
	}

	/// Left-aligns the whole text.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn left_aligned(self) -> Self {
		self.alignment(Alignment::Left)
	}

	/// Center-aligns the whole text.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn centered(self) -> Self {
		self.alignment(Alignment::Center)
	}

	/// Right-aligns the whole text.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn right_aligned(self) -> Self {
		self.alignment(Alignment::Right)
	}

	/// Returns an iterator over the lines.
	pub fn iter(&self) -> core::slice::Iter<'_, Line<'a>> {
		self.lines.iter()
	}

	/// Returns a mutable iterator over the lines.
	pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, Line<'a>> {
		self.lines.iter_mut()
	}

	/// Adds a line to the text.
	pub fn push_line<T: Into<Line<'a>>>(&mut self, line: T) {
		self.lines.push(line.into());
	}

	/// Adds a span to the last line.
	pub fn push_span<T: Into<Span<'a>>>(&mut self, span: T) {
		let span = span.into();
		if let Some(last) = self.lines.last_mut() {
			last.push_span(span);
		} else {
			self.lines.push(Line::from(span));
		}
	}
}

impl UnicodeWidthStr for Text<'_> {
	/// Returns the max width of all the lines.
	fn width(&self) -> usize {
		self.lines
			.iter()
			.map(UnicodeWidthStr::width)
			.max()
			.unwrap_or_default()
	}

	fn width_cjk(&self) -> usize {
		self.lines
			.iter()
			.map(UnicodeWidthStr::width_cjk)
			.max()
			.unwrap_or_default()
	}
}

impl<'a> IntoIterator for Text<'a> {
	type Item = Line<'a>;
	type IntoIter = alloc::vec::IntoIter<Self::Item>;

	fn into_iter(self) -> Self::IntoIter {
		self.lines.into_iter()
	}
}

impl<'a> IntoIterator for &'a Text<'a> {
	type Item = &'a Line<'a>;
	type IntoIter = core::slice::Iter<'a, Line<'a>>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

impl<'a> IntoIterator for &'a mut Text<'a> {
	type Item = &'a mut Line<'a>;
	type IntoIter = core::slice::IterMut<'a, Line<'a>>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter_mut()
	}
}

impl From<String> for Text<'_> {
	fn from(s: String) -> Self {
		Self::raw(s)
	}
}

impl<'a> From<&'a str> for Text<'a> {
	fn from(s: &'a str) -> Self {
		Self::raw(s)
	}
}

impl<'a> From<Cow<'a, str>> for Text<'a> {
	fn from(s: Cow<'a, str>) -> Self {
		Self::raw(s)
	}
}

impl<'a> From<Span<'a>> for Text<'a> {
	fn from(span: Span<'a>) -> Self {
		Self {
			lines: vec![Line::from(span)],
			..Default::default()
		}
	}
}

impl<'a> From<Line<'a>> for Text<'a> {
	fn from(line: Line<'a>) -> Self {
		Self {
			lines: vec![line],
			..Default::default()
		}
	}
}

impl<'a> From<Vec<Line<'a>>> for Text<'a> {
	fn from(lines: Vec<Line<'a>>) -> Self {
		Self {
			lines,
			..Default::default()
		}
	}
}

impl<'a, T> FromIterator<T> for Text<'a>
where
	T: Into<Line<'a>>,
{
	fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
		let lines = iter.into_iter().map(Into::into).collect();
		Self {
			lines,
			..Default::default()
		}
	}
}

impl<'a> core::ops::Add<Line<'a>> for Text<'a> {
	type Output = Self;

	fn add(mut self, line: Line<'a>) -> Self::Output {
		self.push_line(line);
		self
	}
}

/// Adds two `Text` together.
///
/// This ignores the style and alignment of the second `Text`.
impl core::ops::Add<Self> for Text<'_> {
	type Output = Self;

	fn add(mut self, text: Self) -> Self::Output {
		self.lines.extend(text.lines);
		self
	}
}

/// Adds two `Text` together.
///
/// This ignores the style and alignment of the second `Text`.
impl core::ops::AddAssign for Text<'_> {
	fn add_assign(&mut self, rhs: Self) {
		self.lines.extend(rhs.lines);
	}
}

impl<'a> core::ops::AddAssign<Line<'a>> for Text<'a> {
	fn add_assign(&mut self, line: Line<'a>) {
		self.push_line(line);
	}
}

impl<'a, T> Extend<T> for Text<'a>
where
	T: Into<Line<'a>>,
{
	fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
		let lines = iter.into_iter().map(Into::into);
		self.lines.extend(lines);
	}
}

/// A trait for converting a value to a [`Text`].
///
/// This trait is automatically implemented for any type that implements the [`Display`] trait. As
/// such, `ToText` shouldn't be implemented directly: [`Display`] should be implemented instead, and
/// you get the `ToText` implementation for free.
///
/// [`Display`]: std::fmt::Display
pub trait ToText {
	/// Converts the value to a [`Text`].
	fn to_text(&self) -> Text<'_>;
}

/// # Panics
///
/// In this implementation, the `to_text` method panics if the `Display` implementation returns an
/// error. This indicates an incorrect `Display` implementation since `fmt::Write for String` never
/// returns an error itself.
impl<T: fmt::Display> ToText for T {
	fn to_text(&self) -> Text<'_> {
		Text::raw(self.to_string())
	}
}

impl fmt::Display for Text<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if let Some((last, rest)) = self.lines.split_last() {
			for line in rest {
				writeln!(f, "{line}")?;
			}
			write!(f, "{last}")?;
		}
		Ok(())
	}
}

impl Widget for Text<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		Widget::render(&self, area, buf);
	}
}

impl Widget for &Text<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let area = area.intersection(buf.area);
		buf.set_style(area, self.style);
		for (line, line_area) in self.iter().zip(area.rows()) {
			line.render_with_alignment(line_area, buf, self.alignment);
		}
	}
}

impl Styled for Text<'_> {
	type Item = Self;

	fn style(&self) -> Style {
		self.style
	}

	fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
		self.style(style)
	}
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
