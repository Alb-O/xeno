/// A macro for creating a [`Span`] using formatting syntax.
///
/// `span!` is similar to the [`format!`] macro, but it returns a [`Span`] instead of a `String`. In
/// addition, it also accepts an expression for the first argument, which will be converted to a
/// string using the [`format!`] macro.
///
/// If semicolon follows the first argument, then the first argument is a [`Style`] and a styled
/// [`Span`] will be created. Otherwise, the [`Span`] will be created as a raw span (i.e. with style
/// set to `Style::default()`).
///
/// # Examples
///
/// ```rust
/// # use xeno_tui::style::{Color, Modifier, Style, Stylize};
/// use xeno_tui::span;
///
/// let content = "content";
///
/// // expression
/// let span = span!(content);
///
/// // format string
/// let span = span!("test content");
/// let span = span!("test {}", "content");
/// let span = span!("{} {}", "test", "content");
/// let span = span!("test {content}");
/// let span = span!("test {content}", content = "content");
///
/// // with format specifiers
/// let span = span!("test {:4}", 123);
/// let span = span!("test {:04}", 123);
///
/// let style = Style::new().green();
///
/// // styled expression
/// let span = span!(style; content);
///
/// // styled format string
/// let span = span!(style; "test content");
/// let span = span!(style; "test {}", "content");
/// let span = span!(style; "{} {}", "test", "content");
/// let span = span!(style; "test {content}");
/// let span = span!(style; "test {content}", content = "content");
///
/// // accepts any type that is convertible to Style
/// let span = span!(Style::new().green(); "test {content}");
/// let span = span!(Color::Green; "test {content}");
/// let span = span!(Modifier::BOLD; "test {content}");
///
/// // with format specifiers
/// let span = span!(style; "test {:4}", 123);
/// let span = span!(style; "test {:04}", 123);
/// ```
///
/// # Note
///
/// The first parameter must be a formatting specifier followed by a comma OR anything that can be
/// converted into a [`Style`] followed by a semicolon.
///
/// For example, the following will fail to compile:
///
/// ```compile_fail
/// # use xeno_tui::style::Modifier;
/// # use xeno_tui::span;
/// let span = span!(Modifier::BOLD, "hello world");
/// ```
///
/// But this will work:
///
/// ```rust
/// # use xeno_tui::style::{Modifier};
/// # use xeno_tui::span;
/// let span = span!(Modifier::BOLD; "hello world");
/// ```
///
/// The following will fail to compile:
///
/// ```compile_fail
/// # use xeno_tui::style::Modifier;
/// # use xeno_tui::span;
/// let span = span!("hello", "world");
/// ```
///
/// But this will work:
///
/// ```rust
/// # use xeno_tui::span;
/// let span = span!("hello {}", "world");
/// ```
///
/// [`Color`]: crate::style::Color
/// [`Span`]: crate::text::Span
/// [`Style`]: crate::style::Style
/// [`format!`]: std::format!
#[macro_export]
macro_rules! span {
    ($string:literal) => {
        $crate::text::Span::raw(::std::format!($string))
    };
    ($string:literal, $($arg:tt)*) => {
        $crate::text::Span::raw(::std::format!($string, $($arg)*))
    };
    ($expr:expr) => {
        $crate::text::Span::raw(::std::format!("{}", $expr))
    };
    ($style:expr, $($arg:tt)*) => {
        compile_error!("first parameter must be a formatting specifier followed by a comma OR a `Style` followed by a semicolon")
    };
    ($style:expr; $string:literal) => {
        $crate::text::Span::styled(::std::format!($string), $style)
    };
    ($style:expr; $string:literal, $($arg:tt)*) => {
        $crate::text::Span::styled(::std::format!($string, $($arg)*), $style)
    };
    ($style:expr; $expr:expr) => {
        $crate::text::Span::styled(::std::format!("{}", $expr), $style)
    };
}

#[cfg(test)]
mod tests;
