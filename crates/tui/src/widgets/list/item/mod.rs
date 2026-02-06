use crate::style::Style;
use crate::text::Text;

/// A single item in a [`List`]
///
/// The item's height is defined by the number of lines it contains. This can be queried using
/// [`ListItem::height`]. Similarly, [`ListItem::width`] will return the maximum width of all
/// lines.
///
/// You can set the style of an item with [`ListItem::style`] or using the [`Stylize`] trait.
/// This [`Style`] will be combined with the [`Style`] of the inner [`Text`]. The [`Style`]
/// of the [`Text`] will be added to the [`Style`] of the [`ListItem`].
///
/// You can also align a `ListItem` by aligning its underlying [`Text`] and [`Line`]s. For that,
/// see [`Text::alignment`] and [`Line::alignment`]. On a multiline `Text`, one `Line` can override
/// the alignment by setting it explicitly.
///
/// # Examples
///
/// You can create [`ListItem`]s from simple `&str`
///
/// ```rust
/// use xeno_tui::widgets::ListItem;
/// let item = ListItem::new("Item 1");
/// ```
///
/// Anything that can be converted to [`Text`] can be a [`ListItem`].
///
/// ```rust
/// use xeno_tui::text::Line;
/// use xeno_tui::widgets::ListItem;
///
/// let item1: ListItem = "Item 1".into();
/// let item2: ListItem = Line::raw("Item 2").into();
/// ```
///
/// A [`ListItem`] styled with [`Stylize`]
///
/// ```rust
/// use xeno_tui::style::Stylize;
/// use xeno_tui::widgets::ListItem;
///
/// let item = ListItem::new("Item 1").red().on_white();
/// ```
///
/// If you need more control over the item's style, you can explicitly style the underlying
/// [`Text`]
///
/// ```rust
/// use xeno_tui::style::Stylize;
/// use xeno_tui::text::{Span, Text};
/// use xeno_tui::widgets::ListItem;
///
/// let mut text = Text::default();
/// text.extend(["Item".blue(), Span::raw(" "), "1".bold().red()]);
/// let item = ListItem::new(text);
/// ```
///
/// A right-aligned `ListItem`
///
/// ```rust
/// use xeno_tui::text::Text;
/// use xeno_tui::widgets::ListItem;
///
/// ListItem::new(Text::from("foo").right_aligned());
/// ```
///
/// [`List`]: crate::widgets::list::List
/// [`Stylize`]: crate::style::Stylize
/// [`Line`]: crate::text::Line
/// [`Line::alignment`]: crate::text::Line::alignment
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ListItem<'a> {
	/// The text content of this list item.
	pub(crate) content: Text<'a>,
	/// Style applied to the entire item.
	pub(crate) style: Style,
}

impl<'a> ListItem<'a> {
	/// Creates a new [`ListItem`]
	///
	/// The `content` parameter accepts any value that can be converted into [`Text`].
	///
	/// # Examples
	///
	/// You can create [`ListItem`]s from simple `&str`
	///
	/// ```rust
	/// use xeno_tui::widgets::ListItem;
	///
	/// let item = ListItem::new("Item 1");
	/// ```
	///
	/// Anything that can be converted to [`Text`] can be a [`ListItem`].
	///
	/// ```rust
	/// use xeno_tui::text::Line;
	/// use xeno_tui::widgets::ListItem;
	///
	/// let item1: ListItem = "Item 1".into();
	/// let item2: ListItem = Line::raw("Item 2").into();
	/// ```
	///
	/// You can also create multiline items
	///
	/// ```rust
	/// use xeno_tui::widgets::ListItem;
	///
	/// let item = ListItem::new("Multi-line\nitem");
	/// ```
	///
	/// # See also
	///
	/// - [`List::new`](super::List::new) to create a list of items that can be converted to
	///   [`ListItem`]
	pub fn new<T>(content: T) -> Self
	where
		T: Into<Text<'a>>,
	{
		Self {
			content: content.into(),
			style: Style::default(),
		}
	}

	/// Sets the item style
	///
	/// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
	/// your own type that implements [`Into<Style>`]).
	///
	/// This [`Style`] can be overridden by the [`Style`] of the [`Text`] content.
	///
	/// This is a fluent setter method which must be chained or used as it consumes self
	///
	/// # Example
	///
	/// ```rust
	/// use xeno_tui::style::{Style, Stylize};
	/// use xeno_tui::widgets::ListItem;
	///
	/// let item = ListItem::new("Item 1").style(Style::new().red().italic());
	/// ```
	///
	/// `ListItem` also implements the [`Styled`] trait, which means you can use style shorthands
	/// from the [`Stylize`](crate::style::Stylize) trait to set the style of the widget more
	/// concisely.
	///
	/// ```rust
	/// use xeno_tui::style::Stylize;
	/// use xeno_tui::widgets::ListItem;
	///
	/// let item = ListItem::new("Item 1").red().italic();
	/// ```
	///
	/// [`Styled`]: crate::style::Styled
	/// [`ListState`]: crate::widgets::list::ListState
	/// [`Color`]: crate::style::Color
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
		self.style = style.into();
		self
	}

	/// Returns the item height
	///
	/// # Examples
	///
	/// One line item
	///
	/// ```rust
	/// use xeno_tui::widgets::ListItem;
	///
	/// let item = ListItem::new("Item 1");
	/// assert_eq!(item.height(), 1);
	/// ```
	///
	/// Two lines item (note the `\n`)
	///
	/// ```rust
	/// use xeno_tui::widgets::ListItem;
	///
	/// let item = ListItem::new("Multi-line\nitem");
	/// assert_eq!(item.height(), 2);
	/// ```
	pub fn height(&self) -> usize {
		self.content.height()
	}

	/// Returns the max width of all the lines
	///
	/// # Examples
	///
	/// ```rust
	/// use xeno_tui::widgets::ListItem;
	///
	/// let item = ListItem::new("12345");
	/// assert_eq!(item.width(), 5);
	/// ```
	///
	/// ```rust
	/// use xeno_tui::widgets::ListItem;
	///
	/// let item = ListItem::new("12345\n1234567");
	/// assert_eq!(item.width(), 7);
	/// ```
	pub fn width(&self) -> usize {
		self.content.width()
	}
}

impl<'a, T> From<T> for ListItem<'a>
where
	T: Into<Text<'a>>,
{
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

#[cfg(test)]
mod tests;
