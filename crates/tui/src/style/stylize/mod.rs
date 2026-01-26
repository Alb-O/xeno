use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use core::fmt;

use crate::style::{Color, Modifier, Style};
use crate::text::Span;

/// A trait for objects that have a `Style`.
///
/// This trait enables generic code to be written that can interact with any object that has a
/// `Style`. This is used by the `Stylize` trait to allow generic code to be written that can
/// interact with any object that can be styled.
pub trait Styled {
	/// The type of the item that is returned when a style is set.
	type Item;

	/// Returns the style of the object.
	fn style(&self) -> Style;

	/// Sets the style of the object.
	///
	/// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
	/// your own type that implements [`Into<Style>`]).
	fn set_style<S: Into<Style>>(self, style: S) -> Self::Item;
}

/// A helper struct to make it easy to debug using the `Stylize` method names
pub(crate) struct ColorDebug {
	/// Which color property this represents.
	pub kind: ColorDebugKind,
	/// The actual color value.
	pub color: Color,
}

/// Indicates which color property a [`ColorDebug`] represents.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) enum ColorDebugKind {
	/// Text foreground color.
	Foreground,
	/// Text background color.
	Background,
	/// Underline color (requires `underline-color` feature).
	#[cfg(feature = "underline-color")]
	Underline,
}

impl fmt::Debug for ColorDebug {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		#[cfg(feature = "underline-color")]
		let is_underline = self.kind == ColorDebugKind::Underline;
		#[cfg(not(feature = "underline-color"))]
		let is_underline = false;
		if is_underline
			|| matches!(
				self.color,
				Color::Reset | Color::Indexed(_) | Color::Rgb(_, _, _)
			) {
			match self.kind {
				ColorDebugKind::Foreground => write!(f, ".fg(")?,
				ColorDebugKind::Background => write!(f, ".bg(")?,
				#[cfg(feature = "underline-color")]
				ColorDebugKind::Underline => write!(f, ".underline_color(")?,
			}
			write!(f, "Color::{:?}", self.color)?;
			write!(f, ")")?;
			return Ok(());
		}

		match self.kind {
			ColorDebugKind::Foreground => write!(f, ".")?,
			ColorDebugKind::Background => write!(f, ".on_")?,
			// TODO: .underline_color_xxx is not implemented on Stylize yet, but it should be
			#[cfg(feature = "underline-color")]
			ColorDebugKind::Underline => {
				unreachable!("covered by the first part of the if statement")
			}
		}
		match self.color {
			Color::Black => write!(f, "black")?,
			Color::Red => write!(f, "red")?,
			Color::Green => write!(f, "green")?,
			Color::Yellow => write!(f, "yellow")?,
			Color::Blue => write!(f, "blue")?,
			Color::Magenta => write!(f, "magenta")?,
			Color::Cyan => write!(f, "cyan")?,
			Color::Gray => write!(f, "gray")?,
			Color::DarkGray => write!(f, "dark_gray")?,
			Color::LightRed => write!(f, "light_red")?,
			Color::LightGreen => write!(f, "light_green")?,
			Color::LightYellow => write!(f, "light_yellow")?,
			Color::LightBlue => write!(f, "light_blue")?,
			Color::LightMagenta => write!(f, "light_magenta")?,
			Color::LightCyan => write!(f, "light_cyan")?,
			Color::White => write!(f, "white")?,
			_ => unreachable!("covered by the first part of the if statement"),
		}
		write!(f, "()")
	}
}

/// Generates two methods for each color, one for setting the foreground color (`red()`, `blue()`,
/// etc) and one for setting the background color (`on_red()`, `on_blue()`, etc.). Each method sets
/// the color of the style to the corresponding color.
///
/// ```rust,ignore
/// color!(Color::Black, black(), on_black() -> T);
///
/// // generates
///
/// #[doc = "Sets the foreground color to [`black`](Color::Black)."]
/// fn black(self) -> T {
///     self.fg(Color::Black)
/// }
///
/// #[doc = "Sets the background color to [`black`](Color::Black)."]
/// fn on_black(self) -> T {
///     self.bg(Color::Black)
/// }
/// ```
macro_rules! color {
    ( $variant:expr, $color:ident(), $on_color:ident() -> $ty:ty ) => {
        #[doc = concat!("Sets the foreground color to [`", stringify!($color), "`](", stringify!($variant), ").")]
        #[must_use = concat!("`", stringify!($color), "` returns the modified style without modifying the original")]
        fn $color(self) -> $ty {
            self.fg($variant)
        }

        #[doc = concat!("Sets the background color to [`", stringify!($color), "`](", stringify!($variant), ").")]
        #[must_use = concat!("`", stringify!($on_color), "` returns the modified style without modifying the original")]
        fn $on_color(self) -> $ty {
            self.bg($variant)
        }
    };

    (pub const $variant:expr, $color:ident(), $on_color:ident() -> $ty:ty ) => {
        #[doc = concat!("Sets the foreground color to [`", stringify!($color), "`](", stringify!($variant), ").")]
        #[must_use = concat!("`", stringify!($color), "` returns the modified style without modifying the original")]
        pub const fn $color(self) -> $ty {
            self.fg($variant)
        }

        #[doc = concat!("Sets the background color to [`", stringify!($color), "`](", stringify!($variant), ").")]
        #[must_use = concat!("`", stringify!($on_color), "` returns the modified style without modifying the original")]
        pub const fn $on_color(self) -> $ty {
            self.bg($variant)
        }
    };
}

/// Generates a method for a modifier (`bold()`, `italic()`, etc.). Each method sets the modifier
/// of the style to the corresponding modifier.
///
/// # Examples
///
/// ```rust,ignore
/// modifier!(Modifier::BOLD, bold(), not_bold() -> T);
///
/// // generates
///
/// #[doc = "Adds the [`bold`](Modifier::BOLD) modifier."]
/// fn bold(self) -> T {
///     self.add_modifier(Modifier::BOLD)
/// }
///
/// #[doc = "Removes the [`bold`](Modifier::BOLD) modifier."]
/// fn not_bold(self) -> T {
///     self.remove_modifier(Modifier::BOLD)
/// }
/// ```
macro_rules! modifier {
    ( $variant:expr, $modifier:ident(), $not_modifier:ident() -> $ty:ty ) => {
        #[doc = concat!("Adds the [`", stringify!($modifier), "`](", stringify!($variant), ") modifier.")]
        #[must_use = concat!("`", stringify!($modifier), "` returns the modified style without modifying the original")]
        fn $modifier(self) -> $ty {
            self.add_modifier($variant)
        }

        #[doc = concat!("Removes the [`", stringify!($modifier), "`](", stringify!($variant), ") modifier.")]
        #[must_use = concat!("`", stringify!($not_modifier), "` returns the modified style without modifying the original")]
        fn $not_modifier(self) -> $ty {
            self.remove_modifier($variant)
        }
    };

    (pub const $variant:expr, $modifier:ident(), $not_modifier:ident() -> $ty:ty ) => {
        #[doc = concat!("Adds the [`", stringify!($modifier), "`](", stringify!($variant), ") modifier.")]
        #[must_use = concat!("`", stringify!($modifier), "` returns the modified style without modifying the original")]
        pub const fn $modifier(self) -> $ty {
            self.add_modifier($variant)
        }

        #[doc = concat!("Removes the [`", stringify!($modifier), "`](", stringify!($variant), ") modifier.")]
        #[must_use = concat!("`", stringify!($not_modifier), "` returns the modified style without modifying the original")]
        pub const fn $not_modifier(self) -> $ty {
            self.remove_modifier($variant)
        }
    };
}

/// An extension trait for styling objects.
///
/// For any type that implements `Stylize`, the provided methods in this trait can be used to style
/// the type further. This trait is automatically implemented for any type that implements the
/// [`Styled`] trait which e.g.: [`String`], [`&str`], [`Span`], [`Style`] and many Widget types.
///
/// This results in much more ergonomic styling of text and widgets. For example, instead of
/// writing:
///
/// ```rust,ignore
/// let text = Span::styled("Hello", Style::default().fg(Color::Red).bg(Color::Blue));
/// ```
///
/// You can write:
///
/// ```rust,ignore
/// let text = "Hello".red().on_blue();
/// ```
///
/// This trait implements a provided method for every color as both foreground and background
/// (prefixed by `on_`), and all modifiers as both an additive and subtractive modifier (prefixed
/// by `not_`). The `reset()` method is also provided to reset the style.
///
/// # Examples
/// ```ignore
/// use xeno_tui::{
///     style::{Color, Modifier, Style, Stylize},
///     text::Line,
///     widgets::{Block, Paragraph},
/// };
///
/// let span = "hello".red().on_blue().bold();
/// let line = Line::from(vec![
///     "hello".red().on_blue().bold(),
///     "world".green().on_yellow().not_bold(),
/// ]);
/// let paragraph = Paragraph::new(line).italic().underlined();
/// let block = Block::bordered().title("Title").on_white().bold();
/// ```
pub trait Stylize<'a, T>: Sized {
	/// Sets the background color.
	#[must_use = "`bg` returns the modified style without modifying the original"]
	fn bg<C: Into<Color>>(self, color: C) -> T;
	/// Sets the foreground color.
	#[must_use = "`fg` returns the modified style without modifying the original"]
	fn fg<C: Into<Color>>(self, color: C) -> T;
	/// Resets the style.
	#[must_use = "`reset` returns the modified style without modifying the original"]
	fn reset(self) -> T;
	/// Adds a modifier.
	#[must_use = "`add_modifier` returns the modified style without modifying the original"]
	fn add_modifier(self, modifier: Modifier) -> T;
	/// Removes a modifier.
	#[must_use = "`remove_modifier` returns the modified style without modifying the original"]
	fn remove_modifier(self, modifier: Modifier) -> T;

	color!(Color::Black, black(), on_black() -> T);
	color!(Color::Red, red(), on_red() -> T);
	color!(Color::Green, green(), on_green() -> T);
	color!(Color::Yellow, yellow(), on_yellow() -> T);
	color!(Color::Blue, blue(), on_blue() -> T);
	color!(Color::Magenta, magenta(), on_magenta() -> T);
	color!(Color::Cyan, cyan(), on_cyan() -> T);
	color!(Color::Gray, gray(), on_gray() -> T);
	color!(Color::DarkGray, dark_gray(), on_dark_gray() -> T);
	color!(Color::LightRed, light_red(), on_light_red() -> T);
	color!(Color::LightGreen, light_green(), on_light_green() -> T);
	color!(Color::LightYellow, light_yellow(), on_light_yellow() -> T);
	color!(Color::LightBlue, light_blue(), on_light_blue() -> T);
	color!(Color::LightMagenta, light_magenta(), on_light_magenta() -> T);
	color!(Color::LightCyan, light_cyan(), on_light_cyan() -> T);
	color!(Color::White, white(), on_white() -> T);

	modifier!(Modifier::BOLD, bold(), not_bold() -> T);
	modifier!(Modifier::DIM, dim(), not_dim() -> T);
	modifier!(Modifier::ITALIC, italic(), not_italic() -> T);
	modifier!(Modifier::UNDERLINED, underlined(), not_underlined() -> T);
	modifier!(Modifier::SLOW_BLINK, slow_blink(), not_slow_blink() -> T);
	modifier!(Modifier::RAPID_BLINK, rapid_blink(), not_rapid_blink() -> T);
	modifier!(Modifier::REVERSED, reversed(), not_reversed() -> T);
	modifier!(Modifier::HIDDEN, hidden(), not_hidden() -> T);
	modifier!(Modifier::CROSSED_OUT, crossed_out(), not_crossed_out() -> T);
}

impl<T, U> Stylize<'_, T> for U
where
	U: Styled<Item = T>,
{
	fn bg<C: Into<Color>>(self, color: C) -> T {
		let style = self.style().bg(color.into());
		self.set_style(style)
	}

	fn fg<C: Into<Color>>(self, color: C) -> T {
		let style = self.style().fg(color.into());
		self.set_style(style)
	}

	fn add_modifier(self, modifier: Modifier) -> T {
		let style = self.style().add_modifier(modifier);
		self.set_style(style)
	}

	fn remove_modifier(self, modifier: Modifier) -> T {
		let style = self.style().remove_modifier(modifier);
		self.set_style(style)
	}

	fn reset(self) -> T {
		self.set_style(Style::reset())
	}
}

impl<'a> Styled for &'a str {
	type Item = Span<'a>;

	fn style(&self) -> Style {
		Style::default()
	}

	fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
		Span::styled(self, style)
	}
}

impl<'a> Styled for Cow<'a, str> {
	type Item = Span<'a>;

	fn style(&self) -> Style {
		Style::default()
	}

	fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
		Span::styled(self, style)
	}
}

impl Styled for String {
	type Item = Span<'static>;

	fn style(&self) -> Style {
		Style::default()
	}

	fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
		Span::styled(self, style)
	}
}

/// Implements [`Styled`] for a type by converting it to a `Span`.
macro_rules! styled {
	($impl_type:ty) => {
		impl Styled for $impl_type {
			type Item = Span<'static>;

			fn style(&self) -> Style {
				Style::default()
			}

			fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
				Span::styled(self.to_string(), style)
			}
		}
	};
}

styled!(bool);
styled!(char);
styled!(f32);
styled!(f64);
styled!(i8);
styled!(i16);
styled!(i32);
styled!(i64);
styled!(i128);
styled!(isize);
styled!(u8);
styled!(u16);
styled!(u32);
styled!(u64);
styled!(u128);
styled!(usize);

#[cfg(test)]
mod tests;
