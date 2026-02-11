//! Style conversion implementations for various tuple types.

use super::{Color, Modifier, Style};

impl From<Color> for Style {
	/// Creates a new `Style` with the given foreground color.
	///
	/// To specify a foreground and background color, use the `from((fg, bg))` constructor.
	///
	/// # Example
	///
	/// ```rust
	/// use xeno_tui::style::{Color, Style};
	///
	/// let style = Style::from(Color::Red);
	/// ```
	fn from(color: Color) -> Self {
		Self::new().fg(color)
	}
}

impl From<(Color, Color)> for Style {
	/// Creates a new `Style` with the given foreground and background colors.
	///
	/// # Example
	///
	/// ```rust
	/// use xeno_tui::style::{Color, Style};
	///
	/// // red foreground, blue background
	/// let style = Style::from((Color::Red, Color::Blue));
	/// // default foreground, blue background
	/// let style = Style::from((Color::Reset, Color::Blue));
	/// ```
	fn from((fg, bg): (Color, Color)) -> Self {
		Self::new().fg(fg).bg(bg)
	}
}

impl From<Modifier> for Style {
	/// Creates a new `Style` with the given modifier added.
	///
	/// To specify multiple modifiers, use the `|` operator.
	///
	/// To specify modifiers to add and remove, use the `from((add_modifier, sub_modifier))`
	/// constructor.
	///
	/// # Example
	///
	/// ```rust
	/// use xeno_tui::style::{Style, Modifier};
	///
	/// // add bold and italic
	/// let style = Style::from(Modifier::BOLD|Modifier::ITALIC);
	fn from(modifier: Modifier) -> Self {
		Self::new().add_modifier(modifier)
	}
}

impl From<(Modifier, Modifier)> for Style {
	/// Creates a new `Style` with the given modifiers added and removed.
	///
	/// # Example
	///
	/// ```rust
	/// use xeno_tui::style::{Modifier, Style};
	///
	/// // add bold and italic, remove dim
	/// let style = Style::from((Modifier::BOLD | Modifier::ITALIC, Modifier::DIM));
	/// ```
	fn from((add_modifier, sub_modifier): (Modifier, Modifier)) -> Self {
		Self::new().add_modifier(add_modifier).remove_modifier(sub_modifier)
	}
}

impl From<(Color, Modifier)> for Style {
	/// Creates a new `Style` with the given foreground color and modifier added.
	///
	/// To specify multiple modifiers, use the `|` operator.
	///
	/// # Example
	///
	/// ```rust
	/// use xeno_tui::style::{Color, Modifier, Style};
	///
	/// // red foreground, add bold and italic
	/// let style = Style::from((Color::Red, Modifier::BOLD | Modifier::ITALIC));
	/// ```
	fn from((fg, modifier): (Color, Modifier)) -> Self {
		Self::new().fg(fg).add_modifier(modifier)
	}
}

impl From<(Color, Color, Modifier)> for Style {
	/// Creates a new `Style` with the given foreground and background colors and modifier added.
	///
	/// To specify multiple modifiers, use the `|` operator.
	///
	/// # Example
	///
	/// ```rust
	/// use xeno_tui::style::{Color, Modifier, Style};
	///
	/// // red foreground, blue background, add bold and italic
	/// let style = Style::from((Color::Red, Color::Blue, Modifier::BOLD | Modifier::ITALIC));
	/// ```
	fn from((fg, bg, modifier): (Color, Color, Modifier)) -> Self {
		Self::new().fg(fg).bg(bg).add_modifier(modifier)
	}
}

impl From<(Color, Color, Modifier, Modifier)> for Style {
	/// Creates a new `Style` with the given foreground and background colors and modifiers added
	/// and removed.
	///
	/// # Example
	///
	/// ```rust
	/// use xeno_tui::style::{Color, Modifier, Style};
	///
	/// // red foreground, blue background, add bold and italic, remove dim
	/// let style = Style::from((
	///     Color::Red,
	///     Color::Blue,
	///     Modifier::BOLD | Modifier::ITALIC,
	///     Modifier::DIM,
	/// ));
	/// ```
	fn from((fg, bg, add_modifier, sub_modifier): (Color, Color, Modifier, Modifier)) -> Self {
		Self::new().fg(fg).bg(bg).add_modifier(add_modifier).remove_modifier(sub_modifier)
	}
}
