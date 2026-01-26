#![deny(missing_docs)]
#![warn(clippy::pedantic, clippy::nursery, clippy::arithmetic_side_effects)]
use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use unicode_truncate::UnicodeTruncateStr;
use unicode_width::UnicodeWidthStr;

use crate::buffer::Buffer;
use crate::layout::{HorizontalAlignment, Rect};
use crate::style::{Style, Styled};
use crate::text::{Span, StyledGrapheme, Text};
use crate::widgets::Widget;

/// A line of text consisting of one or more [`Span`]s.
///
/// Represents a single line rendered left-to-right. Newlines are removed on creation.
/// Implements [`Widget`] for direct rendering, or use with [`Paragraph`](crate::widgets::Paragraph).
///
/// # Example
///
/// ```rust
/// use xeno_tui::style::{Style, Stylize};
/// use xeno_tui::text::{Line, Span};
///
/// let line = Line::from("Hello").yellow().italic();
/// let line = Line::from(vec![Span::styled("Hello", Style::new().blue()), Span::raw(" world!")]);
/// let line = Line::from("text").centered();
/// ```
#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct Line<'a> {
	/// The style of this line of text.
	pub style: Style,

	/// The alignment of this line of text.
	pub alignment: Option<HorizontalAlignment>,

	/// The spans that make up this line of text.
	pub spans: Vec<Span<'a>>,
}

impl fmt::Debug for Line<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.spans.is_empty() {
			f.write_str("Line::default()")?;
		} else if self.spans.len() == 1 && self.spans[0].style == Style::default() {
			f.write_str(r#"Line::from(""#)?;
			f.write_str(&self.spans[0].content)?;
			f.write_str(r#"")"#)?;
		} else if self.spans.len() == 1 {
			f.write_str("Line::from(")?;
			self.spans[0].fmt(f)?;
			f.write_str(")")?;
		} else {
			f.write_str("Line::from_iter(")?;
			f.debug_list().entries(&self.spans).finish()?;
			f.write_str(")")?;
		}
		self.style.fmt_stylize(f)?;
		match self.alignment {
			Some(HorizontalAlignment::Left) => write!(f, ".left_aligned()"),
			Some(HorizontalAlignment::Center) => write!(f, ".centered()"),
			Some(HorizontalAlignment::Right) => write!(f, ".right_aligned()"),
			None => Ok(()),
		}
	}
}

/// Converts a cow string into a vector of spans, splitting on newlines.
fn cow_to_spans<'a>(content: impl Into<Cow<'a, str>>) -> Vec<Span<'a>> {
	match content.into() {
		Cow::Borrowed(s) => s.lines().map(Span::raw).collect(),
		Cow::Owned(s) => s.lines().map(|v| Span::raw(v.to_string())).collect(),
	}
}

impl<'a> Line<'a> {
	/// Create a line with the default style. Newlines are removed.
	pub fn raw<T>(content: T) -> Self
	where
		T: Into<Cow<'a, str>>,
	{
		Self {
			spans: cow_to_spans(content),
			..Default::default()
		}
	}

	/// Create a line with the given style. Newlines are removed.
	pub fn styled<T, S>(content: T, style: S) -> Self
	where
		T: Into<Cow<'a, str>>,
		S: Into<Style>,
	{
		Self {
			spans: cow_to_spans(content),
			style: style.into(),
			..Default::default()
		}
	}

	/// Sets the spans of this line.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn spans<I>(mut self, spans: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Span<'a>>,
	{
		self.spans = spans.into_iter().map(Into::into).collect();
		self
	}

	/// Sets the style of this line.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
		self.style = style.into();
		self
	}

	/// Sets the alignment. Overrides parent widget alignment.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn alignment(self, alignment: HorizontalAlignment) -> Self {
		Self {
			alignment: Some(alignment),
			..self
		}
	}

	/// Left-aligns this line.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn left_aligned(self) -> Self {
		self.alignment(HorizontalAlignment::Left)
	}

	/// Center-aligns this line.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn centered(self) -> Self {
		self.alignment(HorizontalAlignment::Center)
	}

	/// Right-aligns this line.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn right_aligned(self) -> Self {
		self.alignment(HorizontalAlignment::Right)
	}

	/// Returns the unicode width of the line.
	#[must_use]
	pub fn width(&self) -> usize {
		UnicodeWidthStr::width(self)
	}

	/// Returns an iterator over styled graphemes, with `base_style` merged with line style.
	pub fn styled_graphemes<S: Into<Style>>(
		&'a self,
		base_style: S,
	) -> impl Iterator<Item = StyledGrapheme<'a>> {
		let style = base_style.into().patch(self.style);
		self.spans
			.iter()
			.flat_map(move |span| span.styled_graphemes(style))
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

	/// Returns an iterator over the spans of this line.
	pub fn iter(&self) -> core::slice::Iter<'_, Span<'a>> {
		self.spans.iter()
	}

	/// Returns a mutable iterator over the spans of this line.
	pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, Span<'a>> {
		self.spans.iter_mut()
	}

	/// Adds a span to the line.
	///
	/// `span` can be any type that is convertible into a `Span`. For example, you can pass a
	/// `&str`, a `String`, or a `Span`.
	///
	/// # Examples
	///
	/// ```rust
	/// use xeno_tui::text::{Line, Span};
	///
	/// let mut line = Line::from("Hello, ");
	/// line.push_span(Span::raw("world!"));
	/// line.push_span(" How are you?");
	/// ```
	pub fn push_span<T: Into<Span<'a>>>(&mut self, span: T) {
		self.spans.push(span.into());
	}
}

impl UnicodeWidthStr for Line<'_> {
	fn width(&self) -> usize {
		self.spans.iter().map(UnicodeWidthStr::width).sum()
	}

	fn width_cjk(&self) -> usize {
		self.spans.iter().map(UnicodeWidthStr::width_cjk).sum()
	}
}

impl<'a> IntoIterator for Line<'a> {
	type Item = Span<'a>;
	type IntoIter = alloc::vec::IntoIter<Span<'a>>;

	fn into_iter(self) -> Self::IntoIter {
		self.spans.into_iter()
	}
}

impl<'a> IntoIterator for &'a Line<'a> {
	type Item = &'a Span<'a>;
	type IntoIter = core::slice::Iter<'a, Span<'a>>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

impl<'a> IntoIterator for &'a mut Line<'a> {
	type Item = &'a mut Span<'a>;
	type IntoIter = core::slice::IterMut<'a, Span<'a>>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter_mut()
	}
}

impl From<String> for Line<'_> {
	fn from(s: String) -> Self {
		Self::raw(s)
	}
}

impl<'a> From<&'a str> for Line<'a> {
	fn from(s: &'a str) -> Self {
		Self::raw(s)
	}
}

impl<'a> From<Cow<'a, str>> for Line<'a> {
	fn from(s: Cow<'a, str>) -> Self {
		Self::raw(s)
	}
}

impl<'a> From<Vec<Span<'a>>> for Line<'a> {
	fn from(spans: Vec<Span<'a>>) -> Self {
		Self {
			spans,
			..Default::default()
		}
	}
}

impl<'a> From<Span<'a>> for Line<'a> {
	fn from(span: Span<'a>) -> Self {
		Self::from(vec![span])
	}
}

impl<'a> From<Line<'a>> for String {
	fn from(line: Line<'a>) -> Self {
		line.iter().fold(Self::new(), |mut acc, s| {
			acc.push_str(s.content.as_ref());
			acc
		})
	}
}

impl<'a, T> FromIterator<T> for Line<'a>
where
	T: Into<Span<'a>>,
{
	fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
		Self::from(iter.into_iter().map(Into::into).collect::<Vec<_>>())
	}
}

/// Adds a `Span` to a `Line`, returning a new `Line` with the `Span` added.
impl<'a> core::ops::Add<Span<'a>> for Line<'a> {
	type Output = Self;

	fn add(mut self, rhs: Span<'a>) -> Self::Output {
		self.spans.push(rhs);
		self
	}
}

/// Adds two `Line`s together, returning a new `Text` with the contents of the two `Line`s.
impl<'a> core::ops::Add<Self> for Line<'a> {
	type Output = Text<'a>;

	fn add(self, rhs: Self) -> Self::Output {
		Text::from(vec![self, rhs])
	}
}

impl<'a> core::ops::AddAssign<Span<'a>> for Line<'a> {
	fn add_assign(&mut self, rhs: Span<'a>) {
		self.spans.push(rhs);
	}
}

impl<'a> Extend<Span<'a>> for Line<'a> {
	fn extend<T: IntoIterator<Item = Span<'a>>>(&mut self, iter: T) {
		self.spans.extend(iter);
	}
}

impl Widget for Line<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		Widget::render(&self, area, buf);
	}
}

impl Widget for &Line<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		self.render_with_alignment(area, buf, None);
	}
}

impl Line<'_> {
	/// An internal implementation method for `Widget::render` that allows the parent widget to
	/// define a default alignment, to be used if `Line::alignment` is `None`.
	pub(crate) fn render_with_alignment(
		&self,
		area: Rect,
		buf: &mut Buffer,
		parent_alignment: Option<HorizontalAlignment>,
	) {
		let area = area.intersection(buf.area);
		if area.is_empty() {
			return;
		}
		let area = Rect { height: 1, ..area };
		let line_width = self.width();
		if line_width == 0 {
			return;
		}

		buf.set_style(area, self.style);

		let alignment = self.alignment.or(parent_alignment);

		let area_width = usize::from(area.width);
		let can_render_complete_line = line_width <= area_width;
		if can_render_complete_line {
			let indent_width = match alignment {
				Some(HorizontalAlignment::Center) => (area_width.saturating_sub(line_width)) / 2,
				Some(HorizontalAlignment::Right) => area_width.saturating_sub(line_width),
				Some(HorizontalAlignment::Left) | None => 0,
			};
			let indent_width = u16::try_from(indent_width).unwrap_or(u16::MAX);
			let area = area.indent_x(indent_width);
			render_spans(&self.spans, area, buf, 0);
		} else {
			// There is not enough space to render the whole line. As the right side is truncated by
			// the area width, only truncate the left.
			let skip_width = match alignment {
				Some(HorizontalAlignment::Center) => (line_width.saturating_sub(area_width)) / 2,
				Some(HorizontalAlignment::Right) => line_width.saturating_sub(area_width),
				Some(HorizontalAlignment::Left) | None => 0,
			};
			render_spans(&self.spans, area, buf, skip_width);
		}
	}
}

/// Renders all the spans of the line that should be visible.
fn render_spans(spans: &[Span], mut area: Rect, buf: &mut Buffer, span_skip_width: usize) {
	for (span, span_width, offset) in spans_after_width(spans, span_skip_width) {
		area = area.indent_x(offset);
		if area.is_empty() {
			break;
		}
		span.render(area, buf);
		let span_width = u16::try_from(span_width).unwrap_or(u16::MAX);
		area = area.indent_x(span_width);
	}
}

/// Returns an iterator over the spans that lie after a given skip width from the start of the
/// `Line` (including a partially visible span if the `skip_width` lands within a span).
fn spans_after_width<'a>(
	spans: &'a [Span],
	mut skip_width: usize,
) -> impl Iterator<Item = (Span<'a>, usize, u16)> {
	spans
		.iter()
		.map(|span| (span, span.width()))
		// Filter non visible spans out.
		.filter_map(move |(span, span_width)| {
			// Ignore spans that are completely before the offset. Decrement `span_skip_width` by
			// the span width until we find a span that is partially or completely visible.
			if skip_width >= span_width {
				skip_width = skip_width.saturating_sub(span_width);
				return None;
			}

			// Apply the skip from the start of the span, not the end as the end will be trimmed
			// when rendering the span to the buffer.
			let available_width = span_width.saturating_sub(skip_width);
			skip_width = 0; // ensure the next span is rendered in full
			Some((span, span_width, available_width))
		})
		.map(|(span, span_width, available_width)| {
			if span_width <= available_width {
				// Span is fully visible. Clone here is fast as the underlying content is `Cow`.
				return (span.clone(), span_width, 0u16);
			}
			// Span is only partially visible. As the end is truncated by the area width, only
			// truncate the start of the span.
			let (content, actual_width) = span.content.unicode_truncate_start(available_width);

			// When the first grapheme of the span was truncated, start rendering from a position
			// that takes that into account by indenting the start of the area
			let first_grapheme_offset = available_width.saturating_sub(actual_width);
			let first_grapheme_offset = u16::try_from(first_grapheme_offset).unwrap_or(u16::MAX);
			(
				Span::styled(content, span.style),
				actual_width,
				first_grapheme_offset,
			)
		})
}

/// A trait for converting a value to a [`Line`].
///
/// This trait is automatically implemented for any type that implements the [`Display`] trait. As
/// such, `ToLine` shouln't be implemented directly: [`Display`] should be implemented instead, and
/// you get the `ToLine` implementation for free.
///
/// [`Display`]: std::fmt::Display
pub trait ToLine {
	/// Converts the value to a [`Line`].
	fn to_line(&self) -> Line<'_>;
}

/// # Panics
///
/// In this implementation, the `to_line` method panics if the `Display` implementation returns an
/// error. This indicates an incorrect `Display` implementation since `fmt::Write for String` never
/// returns an error itself.
impl<T: fmt::Display> ToLine for T {
	fn to_line(&self) -> Line<'_> {
		Line::from(self.to_string())
	}
}

impl fmt::Display for Line<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for span in &self.spans {
			write!(f, "{span}")?;
		}
		Ok(())
	}
}

impl Styled for Line<'_> {
	type Item = Self;

	fn style(&self) -> Style {
		self.style
	}

	fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
		self.style(style)
	}
}

#[cfg(test)]
mod tests;
