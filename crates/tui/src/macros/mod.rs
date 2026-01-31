//! Macros for simplifying boilerplate when creating UI elements.
//!
//! # Text Macros
//!
//! The `span!` macro creates raw or styled [`Span`]s.
//!
//! ```rust
//! # use xeno_tui::style::{Color, Modifier, Style, Stylize};
//! # use xeno_tui::span;
//! let name = "world!";
//! let raw_greeting = span!("hello {name}");
//! let styled_greeting = span!(Style::new().green(); "hello {name}");
//! let colored_greeting = span!(Color::Green; "hello {name}");
//! let modified_greeting = span!(Modifier::BOLD; "hello {name}");
//! ```
//!
//! The `line!` macro creates a [`Line`] that contains a sequence of [`Span`]s.
//!
//! ```rust
//! # use xeno_tui::style::{Color, Stylize};
//! # use xeno_tui::{line, span};
//! let name = "world!";
//! let line = line!["hello", format!("{name}")];
//! let line = line!["hello ", span!(Color::Green; "{name}")];
//! let line = line!["Name: ".bold(), "Remy".italic()];
//! let line = line!["bye"; 2];
//! ```
//!
//! The `text!` macro creates a [`Text`] that contains a sequence of [`Line`].
//!
//! ```rust
//! # use xeno_tui::style::{Modifier, Stylize};
//! # use xeno_tui::{span, line, text};
//! let name = "world!";
//! let text = text!["hello", format!("{name}")];
//! let text = text!["bye"; 2];
//! let name = "Bye!!!";
//! let text = text![line!["hello", "world".bold()], span!(Modifier::BOLD; "{name}")];
//! ```
//!
//! # Layout Macros
//!
//! The `constraints!` macro defines an array of [`Constraint`]s:
//!
//! ```rust
//! # use xeno_tui::layout::Layout;
//! # use xeno_tui::constraints;
//! let layout = Layout::horizontal(constraints![==50, ==30%, >=3]);
//! ```
//!
//! The `constraint!` macro defines individual [`Constraint`]s:
//!
//! ```rust
//! # use xeno_tui::layout::Layout;
//! # use xeno_tui::constraint;
//! let layout = Layout::horizontal([constraint!(==50)]);
//! ```
//!
//! The `vertical!` and `horizontal!` macros are a shortcut to defining a [`Layout`]:
//!
//! ```rust
//! # use xeno_tui::layout::Rect;
//! # use xeno_tui::{vertical, horizontal};
//! # let area = Rect { x: 0, y: 0, width: 10, height: 10 };
//! let [top, main, bottom] = vertical![==1, >=1, >=3].areas(area);
//! let [left, main, right] = horizontal![==2, >=1, ==2].areas(main);
//! ```
//!
//! [`Constraint`]: crate::layout::Constraint
//! [`Layout`]: crate::layout::Layout
//! [`Span`]: crate::text::Span
//! [`Line`]: crate::text::Line
//! [`Text`]: crate::text::Text

/// Layout constraint construction macros.
mod layout;
/// Line construction macros.
mod line;
/// Span construction macros.
mod span;
#[cfg(test)]
mod testing;
/// Text construction macros.
mod text;

// Re-export the core crate to use the types in macros
#[doc(hidden)]
pub mod xeno_tui_core {
	pub use crate::*;
}
